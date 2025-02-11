-- Add migration script here

-- Add the "name" column to project
ALTER TABLE projects ADD COLUMN name TEXT NOT NULL DEFAULT '';