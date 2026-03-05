-- Migration: Change volume column type from TEXT to INTEGER
-- This allows proper numeric sorting and removes leading zeros

ALTER TABLE books ALTER COLUMN volume TYPE INTEGER USING volume::integer;
