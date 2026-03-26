ALTER TABLE torrent_downloads
    ADD COLUMN progress       REAL    NOT NULL DEFAULT 0,
    ADD COLUMN download_speed BIGINT  NOT NULL DEFAULT 0,
    ADD COLUMN eta            BIGINT  NOT NULL DEFAULT 0;
