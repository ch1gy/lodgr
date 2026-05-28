-- The notifications table accumulated data until the L6 fix (Phase 3) removed all DB
-- writes from notify.rs. No endpoint reads it; it exists only for cascade-delete
-- compatibility. Dropping it removes dead schema that causes FADP audit confusion.
DROP TABLE IF EXISTS notifications;
