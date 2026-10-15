#!/usr/bin/env bash
# Builds a portable server for Lumina's Quick Add on macOS or Linux:
#
#   packaging/binaries/build.sh <tool> <version> <target>
#
#   tool     php | nginx | redis | postgresql
#   target   macos-aarch64 | macos-x86_64 | linux-x86_64 | linux-aarch64
#
# Writes dist/<tool>-<version>-<target>.tar.gz with one top folder
# (<tool>-<version>/), the layout Lumina looks for after `strip: 1`:
#   php         bin/php-cgi, bin/php     (static, every extension built in)
#   nginx       sbin/nginx               (OpenSSL, PCRE2 and zlib built in)
#   redis       bin/redis-server, bin/redis-cli
#   postgresql  bin/, lib/, share/       (relocatable: libraries found next to it)
#
# Runs on the GitHub-hosted runners (.github/workflows/binaries.yml); the
# runner's system decides what is built, `target` only names the result.
set -euo pipefail

tool=${1:?tool}
version=${2:?version}
target=${3:?target}

root=$(pwd)
work="$root/work/$tool-$version-$target"
name="$tool-$version"
stage="$work/stage/$name"
dist="$root/dist"
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)
os=${target%%-*}

rm -rf "$work"
mkdir -p "$work/src" "$stage" "$dist"

# macOS: run on every version Lumina supports.
export MACOSX_DEPLOYMENT_TARGET=11.0

fetch() { # url file
  curl -fsSL --retry 3 --retry-delay 5 -o "$2" "$1"
}

# Latest release tag of a GitHub repository (GITHUB_TOKEN avoids rate limits).
latest_tag() { # owner/repo [tag-filter-regex]
  local auth=()
  [ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
  curl -fsSL ${auth[@]+"${auth[@]}"} "https://api.github.com/repos/$1/releases?per_page=30" |
    python3 -c 'import json,re,sys
pat=re.compile(sys.argv[1])
for r in json.load(sys.stdin):
    if not r["prerelease"] and not r["draft"] and pat.search(r["tag_name"]):
        print(r["tag_name"]); break' "${2:-.}"
}

build_php() {
  # Extensions compiled into the binary (static: nothing to load from ext/).
  local exts="bcmath,bz2,calendar,ctype,curl,dom,exif,fileinfo,filter,ftp,gd,iconv,intl,mbregex,mbstring,mysqli,mysqlnd,opcache,openssl,pcntl,pdo,pdo_mysql,pdo_pgsql,pdo_sqlite,pgsql,phar,posix,redis,session,simplexml,sockets,sodium,sqlite3,tokenizer,xml,xmlreader,xmlwriter,xsl,zip,zlib"
  local spc_version=2.8.5 spc_os spc_arch
  spc_os=$os
  spc_arch=${target#*-}
  cd "$work"
  fetch "https://github.com/crazywhalecc/static-php-cli/releases/download/$spc_version/spc-$spc_os-$spc_arch.tar.gz" spc.tar.gz
  tar -xzf spc.tar.gz
  chmod +x spc
  ./spc doctor --auto-fix
  ./spc download --with-php="$version" --for-extensions="$exts" --prefer-pre-built
  # Lumina always passes its own php.ini with -c.
  ./spc build "$exts" --build-cli --build-cgi
  mkdir -p "$stage/bin"
  cp buildroot/bin/php buildroot/bin/php-cgi "$stage/bin/"
  cp buildroot/license/* "$stage/" 2>/dev/null || true
  "$stage/bin/php-cgi" -v
  "$stage/bin/php" -m
}

build_nginx() {
  local pcre zlib openssl
  pcre=$(latest_tag PCRE2Project/pcre2 '^pcre2-10\.')
  zlib=$(latest_tag madler/zlib '^v1\.')
  openssl=$(latest_tag openssl/openssl '^openssl-3\.5\.')
  echo "nginx $version with $pcre, zlib $zlib, $openssl"
  cd "$work/src"
  fetch "https://nginx.org/download/nginx-$version.tar.gz" nginx.tar.gz
  fetch "https://github.com/PCRE2Project/pcre2/releases/download/$pcre/$pcre.tar.gz" pcre2.tar.gz
  fetch "https://github.com/madler/zlib/releases/download/$zlib/zlib-${zlib#v}.tar.gz" zlib.tar.gz
  fetch "https://github.com/openssl/openssl/releases/download/$openssl/$openssl.tar.gz" openssl.tar.gz
  for f in nginx pcre2 zlib openssl; do mkdir "$f" && tar -xzf "$f.tar.gz" -C "$f" --strip-components=1; done
  cd nginx
  # Lumina runs nginx with its own prefix (-p) and config (-c).
  ./configure \
    --prefix=. \
    --sbin-path=sbin/nginx \
    --conf-path=conf/nginx.conf \
    --error-log-path=logs/error.log \
    --http-log-path=logs/access.log \
    --pid-path=logs/nginx.pid \
    --http-client-body-temp-path=temp/client_body \
    --http-proxy-temp-path=temp/proxy \
    --http-fastcgi-temp-path=temp/fastcgi \
    --http-uwsgi-temp-path=temp/uwsgi \
    --http-scgi-temp-path=temp/scgi \
    --with-pcre="$work/src/pcre2" --with-pcre-jit \
    --with-zlib="$work/src/zlib" \
    --with-openssl="$work/src/openssl" \
    --with-threads \
    --with-http_ssl_module \
    --with-http_v2_module \
    --with-http_realip_module \
    --with-http_gzip_static_module \
    --with-http_gunzip_module \
    --with-http_stub_status_module \
    --with-http_sub_module \
    --with-stream \
    --with-stream_ssl_module
  make -j"$jobs"
  mkdir -p "$stage/sbin" "$stage/conf"
  cp objs/nginx "$stage/sbin/"
  cp conf/* "$stage/conf/"
  cp LICENSE "$stage/LICENSE"
  "$stage/sbin/nginx" -V
}

build_redis() {
  cd "$work/src"
  fetch "https://github.com/redis/redis/archive/refs/tags/$version.tar.gz" redis.tar.gz
  mkdir redis && tar -xzf redis.tar.gz -C redis --strip-components=1
  cd redis
  local malloc=()
  [ "$os" = macos ] && malloc=(MALLOC=libc)
  make -j"$jobs" BUILD_TLS=no USE_SYSTEMD=no ${malloc[@]+"${malloc[@]}"}
  mkdir -p "$stage/bin"
  cp src/redis-server src/redis-cli src/redis-benchmark "$stage/bin/"
  cp redis.conf "$stage/redis.conf.example"
  cp LICENSE* COPYING* "$stage/" 2>/dev/null || true
  "$stage/bin/redis-server" --version
}

build_postgresql() {
  local prefix="$work/prefix"
  cd "$work/src"
  fetch "https://ftp.postgresql.org/pub/source/v$version/postgresql-$version.tar.bz2" pg.tar.bz2
  mkdir pg && tar -xjf pg.tar.bz2 -C pg --strip-components=1
  cd pg
  ./configure --prefix="$prefix" --without-icu --without-readline --with-zlib
  make -j"$jobs" world-bin
  make install-world-bin
  cp -R "$prefix/." "$stage/"
  cp COPYRIGHT "$stage/"
  # Find the bundled libraries next to the binaries, wherever Lumina unpacks them.
  if [ "$os" = macos ]; then
    local f dep
    for f in "$stage"/bin/* "$stage"/lib/*.dylib "$stage"/lib/postgresql/*.dylib "$stage"/lib/postgresql/*.so; do
      if [ ! -f "$f" ] || [ -L "$f" ]; then continue; fi
      if ! file "$f" | grep -q Mach-O; then continue; fi
      for dep in $(otool -L "$f" | awk 'NR>1 {print $1}' | grep "^$prefix/lib/" || true); do
        case "$f" in
          "$stage"/bin/*) install_name_tool -change "$dep" "@executable_path/../lib/${dep##*/}" "$f" ;;
          "$stage"/lib/postgresql/*) install_name_tool -change "$dep" "@loader_path/../${dep##*/}" "$f" ;;
          *) install_name_tool -change "$dep" "@loader_path/${dep##*/}" "$f" ;;
        esac
      done
      case "$f" in
        "$stage"/lib/*.dylib) install_name_tool -id "@rpath/${f##*/}" "$f" ;;
      esac
      codesign --force --sign - "$f"
    done
  else
    local f
    # shellcheck disable=SC2016 # $ORIGIN is for the dynamic loader, not the shell.
    for f in "$stage"/bin/*; do
      if file "$f" | grep -q ELF; then patchelf --set-rpath '$ORIGIN/../lib' "$f"; fi
    done
    for f in "$stage"/lib/*.so* "$stage"/lib/postgresql/*.so; do
      if [ ! -f "$f" ] || [ -L "$f" ]; then continue; fi
      # shellcheck disable=SC2016
      if file "$f" | grep -q ELF; then patchelf --set-rpath '$ORIGIN:$ORIGIN/..' "$f"; fi
    done
  fi
  # Prove the build works from its new place: create a cluster, start, query, stop.
  local data="$work/pgdata" port=54329
  "$stage/bin/initdb" -D "$data" -U postgres --auth=trust --encoding=UTF8 --no-locale >/dev/null
  "$stage/bin/pg_ctl" -D "$data" -o "-p $port -c listen_addresses=127.0.0.1 -c unix_socket_directories=" -w start
  "$stage/bin/psql" -h 127.0.0.1 -p "$port" -U postgres -tAc 'select version()'
  "$stage/bin/pg_ctl" -D "$data" -m fast -w stop
}

"build_$tool"

# macOS: every executable and library carries a valid (ad-hoc) signature.
if [ "$os" = macos ]; then
  find "$stage" -type f \( -perm -u+x -o -name '*.dylib' -o -name '*.so' \) -print0 |
    while IFS= read -r -d '' f; do
      if file "$f" | grep -q Mach-O; then codesign --force --sign - "$f"; fi
    done
fi

cd "$work/stage"
tar -czf "$dist/$name-$target.tar.gz" "$name"
echo "built $dist/$name-$target.tar.gz"
