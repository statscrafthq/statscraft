#!/bin/sh
# Keeps vendor and product names of other companies out of the repository.
# Prints file:line (or commit: line) for every hit and exits 1 when there is one.
#
#   scripts/check-terms.sh --staged          staged files (content, binary too) and their paths
#   scripts/check-terms.sh --message FILE    a commit message
#   scripts/check-terms.sh --tree            every tracked file (binary too) and every path
#   scripts/check-terms.sh --history RANGE   messages, author and committer of a range (default HEAD;
#                                            several words are passed to git log as they are)
#   scripts/check-terms.sh --refs NAME...    ref names, for example the branches of a push
#
# The pattern is spelled with bracket classes so this file does not match itself. It catches the
# product name anywhere in any case, and the company name as a word or an identifier part (an
# uppercase run, or followed by digits or an underscore), but not words like libm.
set -eu

PATTERN='[sS][pP][sS][sS]|(^|[^a-zA-Z])[iI][bB][mM]([^a-zA-Z]|$)|[I][B][M]'

# Temporary allow-list: these files hold wire identifiers that the external OpenStatSpec spec
# defines. They go away when the spec renames them.
ALLOW='crates/openstatspec/src/wire.rs'

cd "$(git rev-parse --show-toplevel)"

# The top-level README may name the product on one line, so searches find the project (owner
# 2026-11-12). The company name stays out of it, and a second line with the product name fails.
COMPANY='(^|[^a-zA-Z])[iI][bB][mM]([^a-zA-Z]|$)|[I][B][M]'

# Drops "path:..." lines whose path is allow-listed, and the one README line allowed above.
allowed() {
    awk -v allow="$ALLOW" -v company="$COMPANY" 'BEGIN { n = split(allow, a, " ") }
        { for (i = 1; i <= n; i++) if (index($0, a[i] ":") == 1) next }
        index($0, "README.md:") == 1 { r[++k] = $0; next }
        { print }
        END { for (i = 1; i <= k; i++) if (k > 1 || substr(r[i], index(substr(r[i], 11), ":") + 11) ~ company) print r[i] }'
}

# Shortens hits so binary lines stay readable.
show() {
    cut -c1-200 | tr -c '[:print:]\n' '?'
}

case "${1:-}" in
--staged)
    hits=$(
        git diff --cached --name-only --diff-filter=ACMR -z |
            xargs -0 -r git grep --cached -n -a -E "$PATTERN" -- 2>/dev/null | allowed
        git diff --cached --name-only --diff-filter=ACMR | grep -E "$PATTERN" | sed 's/$/: file name/'
    ) || true
    hits=$(printf '%s\n' "$hits" | sort -u | sed '/^$/d')
    ;;
--message)
    file=${2:?--message needs a file}
    # git drops comment lines and everything below the scissors line
    hits=$(awk '/^# -+ >8 -+$/ { exit } /^#/ { next } { print NR ": " $0 }' "$file" |
        grep -E "$PATTERN" | sed "s|^|commit message line |") || true
    ;;
--tree)
    hits=$(
        git grep -n -a -E "$PATTERN" -- . | allowed
        git ls-files | grep -E "$PATTERN" | sed 's/$/: file name/'
    ) || true
    ;;
--history)
    shift
    [ $# -gt 0 ] || set -- HEAD
    hits=$(git log --format='%x01%h%n%an <%ae>%n%cn <%ce>%n%B' "$@" |
        awk '/^\001/ { c = substr($0, 2); next } { print c ": " $0 }' |
        grep -E "$PATTERN") || true
    ;;
--refs)
    shift
    hits=$(printf '%s\n' "$@" | grep -E "$PATTERN" | sed 's/^/ref name: /') || true
    ;;
*)
    echo "usage: $0 --staged | --message FILE | --tree | --history [RANGE] | --refs NAME..." >&2
    exit 2
    ;;
esac

if [ -n "$hits" ]; then
    printf '%s\n' "$hits" | show
    echo "check-terms: remove the names above (see scripts/check-terms.sh)" >&2
    exit 1
fi
