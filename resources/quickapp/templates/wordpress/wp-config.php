<?php
/**
 * WordPress configuration, written by Lumina QuickApp for {domain}.
 *
 * The database runs on Lumina's local server. The keys and salts below were
 * generated for this site only; change them at any time to log everyone out.
 *
 * @link https://developer.wordpress.org/advanced-administration/wordpress/wp-config/
 */

// ** Database settings ** //
define( 'DB_NAME', '{db}' );
define( 'DB_USER', '{db_user}' );
define( 'DB_PASSWORD', '{db_password}' );
define( 'DB_HOST', '{db_host}:{db_port}' );
define( 'DB_CHARSET', 'utf8mb4' );
define( 'DB_COLLATE', '' );

// ** Authentication unique keys and salts ** //
define( 'AUTH_KEY',         '{random:64}' );
define( 'SECURE_AUTH_KEY',  '{random:64}' );
define( 'LOGGED_IN_KEY',    '{random:64}' );
define( 'NONCE_KEY',        '{random:64}' );
define( 'AUTH_SALT',        '{random:64}' );
define( 'SECURE_AUTH_SALT', '{random:64}' );
define( 'LOGGED_IN_SALT',   '{random:64}' );
define( 'NONCE_SALT',       '{random:64}' );

$table_prefix = 'wp_';

// A local development site.
define( 'WP_ENVIRONMENT_TYPE', 'local' );
define( 'WP_DEBUG', false );

/* That's all, stop editing! Happy publishing. */

/** Absolute path to the WordPress directory. */
if ( ! defined( 'ABSPATH' ) ) {
	define( 'ABSPATH', __DIR__ . '/' );
}

/** Sets up WordPress vars and included files. */
require_once ABSPATH . 'wp-settings.php';
