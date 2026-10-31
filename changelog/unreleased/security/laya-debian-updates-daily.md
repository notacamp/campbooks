- **The Laya model server image carries Debian's latest security fixes.** Its build now
  applies Debian's updates again every day instead of reusing an earlier build's, so a fix
  Debian has released (such as OpenSSL's for CVE-2026-84782) reaches the published
  `campbooks-laya` image with the next build, and CI fails a Laya image that misses one.
