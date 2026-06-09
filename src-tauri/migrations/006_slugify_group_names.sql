-- Fix existing group names to use slug format (lowercase, hyphens, no spaces/special chars)
-- e.g. "GLM (auto)" → "glm-auto", "minmax (auto)" → "minmax-auto"

-- First pass: basic replacements
UPDATE groups SET name = REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(
    LOWER(name),
    ' ', '-'
  ), '（', '-'
  ), '（', '-'
  ), '（', '-'
  ), '（', '-'
) WHERE 1=0;

-- Proper cleanup: strip all non-slug chars, collapse hyphens
UPDATE groups SET name = TRIM(BOTH '-' FROM
  REPLACE(
    REPLACE(
      REPLACE(
        REPLACE(
          REPLACE(LOWER(name), ' ', '-'),
          '（', '-'
        ),
        '）', '-'
      ),
      '(', '-'
    ),
    ')', '-'
  )
);

-- Collapse multiple consecutive hyphens (run a few times for safety)
UPDATE groups SET name = REPLACE(name, '--', '-') WHERE name LIKE '%--%';
UPDATE groups SET name = REPLACE(name, '--', '-') WHERE name LIKE '%--%';
UPDATE groups SET name = REPLACE(name, '--', '-') WHERE name LIKE '%--%';

-- Trim leading/trailing hyphens
UPDATE groups SET name = TRIM(BOTH '-' FROM name) WHERE name LIKE '-%' OR name LIKE '%-';
