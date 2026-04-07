-- Add tracker URL to torrents (link to the torrent's page on the indexer)
ALTER TABLE torrents ADD COLUMN tracker_url TEXT;
