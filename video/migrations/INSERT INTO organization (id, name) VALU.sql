INSERT INTO organizations (id, name) VALUES ('018A24BB-B729-9775-B1F6-3FBC3A3254CA', 'test') ON CONFLICT DO NOTHING;

INSERT INTO rooms (organization_id, id, stream_key) VALUES ('018A24BB-B729-9775-B1F6-3FBC3A3254CA', '018A24BB-B729-9775-B1F6-3FBC3A3254CA', '018A24BBB7299775B1F63FBC3A3254CA') ON CONFLICT DO NOTHING;