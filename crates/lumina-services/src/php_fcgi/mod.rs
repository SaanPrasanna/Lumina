//! PHP FastCGI: spawns `php-cgi -b 127.0.0.1:<port>` with PHP_FCGI_CHILDREN /
//! PHP_FCGI_MAX_REQUESTS, enforces the child minimum, and fully terminates the
//! old process tree on PHP version change.

pub mod env;
pub mod versions;
