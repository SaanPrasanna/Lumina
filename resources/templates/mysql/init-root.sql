-- Run once by Lumina while initialising a new MySQL data directory.
-- `mysqld --initialize-insecure` only creates root@localhost; with
-- skip-name-resolve, TCP clients (PHP, GUI tools) arrive as root@127.0.0.1 or
-- root@::1. Local development only: the server listens on 127.0.0.1.
CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '';
GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;
CREATE USER IF NOT EXISTS 'root'@'::1' IDENTIFIED BY '';
GRANT ALL PRIVILEGES ON *.* TO 'root'@'::1' WITH GRANT OPTION;
FLUSH PRIVILEGES;
