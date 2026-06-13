-- Add is_stream flag to proxy_log to explicitly mark streaming (SSE) requests.
-- Replaces the implicit response_body == "[stream]" sentinel; streaming logs now
-- store real aggregated SSE content in response_body / user_response_body.

ALTER TABLE proxy_log ADD COLUMN is_stream INTEGER NOT NULL DEFAULT 0;
