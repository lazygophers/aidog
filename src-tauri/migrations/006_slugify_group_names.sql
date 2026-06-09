-- Fix existing group names to use slug format (lowercase, hyphens, no spaces/special chars)
-- e.g. "GLM (auto)" → "glm-auto", "minmax (auto)" → "minmax-auto"

-- Replace spaces, parens (full-width and half-width) with hyphens, lowercase
UPDATE groups SET name = REPLACE(REPLACE(REPLACE(REPLACE(
    LOWER(name),
    ' ', '-'
  ), '（', '-'
  ), '(', '-'
  ), '）', '-'
) WHERE name LIKE '% %' OR name LIKE '%(%' OR name LIKE '%（%' OR name LIKE '%）%' OR name != LOWER(name);

-- Also strip closing half-width paren
UPDATE groups SET name = REPLACE(name, ')', '-') WHERE name LIKE '%)%';

-- Collapse multiple consecutive hyphens (run a few times for safety)
UPDATE groups SET name = REPLACE(name, '--', '-') WHERE name LIKE '%--%';
UPDATE groups SET name = REPLACE(name, '--', '-') WHERE name LIKE '%--%';
UPDATE groups SET name = REPLACE(name, '--', '-') WHERE name LIKE '%--%';

-- Trim leading hyphens (SQLite: LTRIM with chars)
UPDATE groups SET name = LTRIM(name, '-') WHERE name LIKE '-%';
-- Trim trailing hyphens
UPDATE groups SET name = RTRIM(name, '-') WHERE name LIKE '%-';
