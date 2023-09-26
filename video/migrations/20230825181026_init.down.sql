-- Add down migration script here

DROP TABLE IF EXISTS organizations CASCADE;
DROP TABLE IF EXISTS access_tokens CASCADE;
DROP TABLE IF EXISTS s3_buckets CASCADE;
DROP TABLE IF EXISTS transcoding_configs CASCADE;
DROP TABLE IF EXISTS recording_configs CASCADE;
DROP TABLE IF EXISTS rooms CASCADE;
DROP TABLE IF EXISTS recordings CASCADE;
DROP TABLE IF EXISTS recording_thumbnails CASCADE;
DROP TABLE IF EXISTS recording_renditions CASCADE;
DROP TABLE IF EXISTS recording_rendition_segments CASCADE;
DROP VIEW IF EXISTS recording_rendition_manifests;
DROP TABLE IF EXISTS playback_key_pairs CASCADE;
DROP TABLE IF EXISTS playback_key_pairs CASCADE;
DROP TABLE IF EXISTS playback_sessions CASCADE;
DROP TABLE IF EXISTS session_token_revokes CASCADE;

DROP TYPE IF EXISTS rendition;
DROP TYPE IF EXISTS playback_session_device;
DROP TYPE IF EXISTS playback_session_platform;
DROP TYPE IF EXISTS playback_session_browser;
DROP TYPE IF EXISTS room_status;