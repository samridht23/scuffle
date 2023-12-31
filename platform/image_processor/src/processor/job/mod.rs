use std::path::{Path, PathBuf};
use std::sync::Arc;

use file_format::FileFormat;
use futures::FutureExt;
use prost::Message;
use tokio::select;
use tokio::sync::SemaphorePermit;

use self::decoder::DecoderBackend;
use super::error::{ProcessorError, Result};
use super::utils;
use crate::database;
use crate::global::ImageProcessorGlobal;
use crate::processor::utils::refresh_job;

pub(crate) mod decoder;
pub(crate) mod encoder;
pub(crate) mod frame;
pub(crate) mod frame_deduplicator;
pub(crate) mod libavif;
pub(crate) mod libwebp;
pub(crate) mod process;
pub(crate) mod resize;
pub(crate) mod smart_object;

pub(crate) struct Job<'a, G: ImageProcessorGlobal> {
	pub(crate) global: &'a Arc<G>,
	pub(crate) job: database::Job,
	pub(crate) working_directory: std::path::PathBuf,
}

pub async fn handle_job(
	global: &Arc<impl ImageProcessorGlobal>,
	parent_directory: &Path,
	_ticket: SemaphorePermit<'_>,
	job: database::Job,
) -> Result<()> {
	let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));

	let job_id = job.id.0;
	let max_processing_time_ms = job.task.limits.as_ref().map(|l| l.max_processing_time_ms);

	let working_directory = parent_directory.join(job_id.to_string());

	let job = Job {
		global,
		job,
		working_directory,
	};

	let time_limit = async {
		if let Some(max_processing_time_ms) = max_processing_time_ms {
			tokio::time::sleep(std::time::Duration::from_millis(max_processing_time_ms as u64)).await;
			Err(ProcessorError::TimeLimitExceeded)
		} else {
			Ok(())
		}
	};

	let mut process = std::pin::pin!(job.process());
	let time_limit = std::pin::pin!(time_limit);
	let mut time_limit = time_limit.fuse();

	loop {
		select! {
			_ = interval.tick() => {
				refresh_job(global, job_id).await?;
			},
			Err(e) = &mut time_limit => {
				return Err(e);
			},
			r = &mut process => {
				return r;
			},
		}
	}
}

impl<'a, G: ImageProcessorGlobal> Job<'a, G> {
	async fn download_source(&self) -> Result<PathBuf> {
		let dest = self.working_directory.join("input");

		tracing::info!(
			"downloading {}/{} to {:?}",
			self.global.config().source_bucket.name,
			self.job.id.to_string(),
			dest
		);

		let mut fs = tokio::fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&dest)
			.await
			.map_err(ProcessorError::FileCreate)?;

		let status = self
			.global
			.s3_source_bucket()
			.get_object_to_writer(&self.job.task.input_path, &mut fs)
			.await
			.map_err(ProcessorError::S3Download)?;

		if (200..299).contains(&status) {
			Ok(dest)
		} else {
			Err(ProcessorError::S3Download(s3::error::S3Error::HttpFail))
		}
	}

	pub(crate) async fn process(self) -> Result<()> {
		tokio::fs::create_dir_all(&self.working_directory)
			.await
			.map_err(ProcessorError::DirectoryCreate)?;

		let input_path = self.download_source().await?;

		let backend = DecoderBackend::from_format(FileFormat::from_file(&input_path).map_err(ProcessorError::FileFormat)?)?;

		let url_prefix = format!("result/{}{}", self.job.task.output_prefix, self.job.id);

		let job_c = self.job.clone();
		let images = tokio::task::spawn_blocking(move || process::process_job(backend, &input_path, &job_c))
			.await
			.unwrap_or_else(|e| {
				tracing::error!(error = %e, "failed to spawn blocking task");
				Err(ProcessorError::BlockingTaskSpawn)
			})?;

		for image in images.images.iter() {
			// image upload
			let url = image.url(&url_prefix);
			tracing::info!("uploading result to {}/{}", self.global.config().target_bucket.name, url);
			let resp = self
				.global
				.s3_target_bucket()
				.put_object_with_content_type(url, &image.data, image.content_type())
				.await
				.map_err(ProcessorError::S3Upload)?;

			if !(200..299).contains(&resp.status_code()) {
				return Err(ProcessorError::S3Upload(s3::error::S3Error::HttpFail));
			}
		}
		// job completion
		self.global
			.nats()
			.publish(
				self.job.task.callback_subject.clone(),
				pb::scuffle::platform::internal::events::ProcessedImage {
					job_id: Some(self.job.id.0.into()),
					variants: images
						.images
						.iter()
						.map(|i| pb::scuffle::platform::internal::types::ProcessedImageVariant {
							path: i.url(&url_prefix),
							format: i.request.1.into(),
							width: i.width as u32,
							height: i.height as u32,
							byte_size: i.data.len() as u32,
							scale: i.request.0 as u32,
						})
						.collect(),
				}
				.encode_to_vec()
				.into(),
			)
			.await
			.map_err(|e| {
				tracing::error!(err = %e, "failed to publish event");
				e
			})?;

		// delete job
		utils::delete_job(self.global, self.job.id.0).await?;

		Ok(())
	}
}

impl<'a, G: ImageProcessorGlobal> Drop for Job<'a, G> {
	fn drop(&mut self) {
		let working_directory = self.working_directory.clone();
		tokio::spawn(async move {
			tokio::fs::remove_dir_all(working_directory).await.unwrap_or_else(|e| {
				tracing::error!(error = %e, "failed to remove working directory");
			});
		});
	}
}