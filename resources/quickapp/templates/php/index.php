<?php
// {name}: created by Lumina QuickApp. Replace this file with your own code.
$php = PHP_VERSION;
$extensions = get_loaded_extensions();
sort($extensions);
?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{name}</title>
  <style>
    body { font: 16px/1.6 system-ui, sans-serif; max-width: 42rem; margin: 4rem auto; padding: 0 1rem; color: #1f2937; }
    code { background: #f3f4f6; padding: .1rem .3rem; border-radius: .25rem; }
    @media (prefers-color-scheme: dark) { body { background: #111827; color: #e5e7eb; } code { background: #1f2937; } }
  </style>
</head>
<body>
  <h1>{name}</h1>
  <p>This site is served from <code><?= htmlspecialchars(__DIR__) ?></code> on PHP <?= htmlspecialchars($php) ?>.</p>
  <p>Edit <code>index.php</code> to get started.</p>
  <details>
    <summary>Loaded extensions (<?= count($extensions) ?>)</summary>
    <p><?= htmlspecialchars(implode(', ', $extensions)) ?></p>
  </details>
</body>
</html>
