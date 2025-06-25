-- Add migration script here

DELETE FROM user_project_relations
WHERE ctid NOT IN (
  SELECT MIN(ctid)
  FROM user_project_relations
  GROUP BY user_id, project_id
);

-- Step 2: Add the UNIQUE constraint
ALTER TABLE public.user_project_relations
ADD CONSTRAINT user_project_relations_user_id_project_id_key
UNIQUE (user_id, project_id);
