-- Fix existing group names to use slug format (lowercase, hyphens, no spaces/special chars)
-- e.g. "GLM (auto)" → "glm-auto", "minmax (auto)" → "minmax-auto"

UPDATE groups SET name = REPLACE(REPLACE(REPLACE(REPLACE(
    LOWER(name),
    ' ', '-'
  ), '（', '-'
  ), '）', ''
  ), '(', '-'
) WHERE name LIKE '% %' OR name LIKE '%(%' OR name LIKE '%（%' OR name != LOWER(name);
