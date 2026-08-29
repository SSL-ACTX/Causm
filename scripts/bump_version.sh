#!/bin/sh
set -eu

# scripts/bump_version.sh
# Consistent SemVer version management for Causm workspace, crates, and documentation.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CURRENT_VERSION="0.1.0-alpha.1"

# Extract current workspace version from root Cargo.toml if available
if [ -f "$ROOT_DIR/Cargo.toml" ]; then
    DETECTED_VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | sed -E 's/version = "(.*)"/\1/' || echo "$CURRENT_VERSION")
    if [ -n "$DETECTED_VERSION" ]; then
        CURRENT_VERSION="$DETECTED_VERSION"
    fi
fi

usage() {
    echo "Usage: $0 <command> [arguments]"
    echo ""
    echo "Commands:"
    echo "  current                    Display the current detected version"
    echo "  set <new_version>          Set an exact version (e.g. 0.1.0-alpha.2, 0.1.0-beta.1, 0.1.0)"
    echo "  bump <major|minor|patch|alpha|beta|rc|release>  Bump the version according to SemVer rules"
    echo "  branch [custom_prefix]     Create a git branch for the current version (e.g. release/v0.1.0-alpha.1)"
    echo ""
    echo "Examples:"
    echo "  $0 current"
    echo "  $0 set 0.1.0-alpha.2"
    echo "  $0 bump alpha"
    echo "  $0 branch"
    echo "  $0 branch release"
    exit 1
}

create_branch() {
    prefix="${1:-release}"
    branch_name="${prefix}/v${CURRENT_VERSION}"
    echo "==> Creating git branch: $branch_name"
    if git rev-parse --verify "$branch_name" >/dev/null 2>&1; then
        echo "Branch '$branch_name' already exists. Checking out..."
        git checkout "$branch_name"
    else
        git checkout -b "$branch_name"
    fi
    echo "  [✓] Switched to branch '$branch_name'"
}

update_files() {
    target_ver="$1"
    echo "==> Updating Causm version: $CURRENT_VERSION -> $target_ver"

    # 1. Update root Cargo.toml workspace.package version
    if [ -f "$ROOT_DIR/Cargo.toml" ]; then
        sed -i -E "s/^(version = \")[^\"]+(\")$/\1$target_ver\2/" "$ROOT_DIR/Cargo.toml"
        echo "  [✓] Updated Cargo.toml (workspace)"
    fi

    # 2. Update crate-level Cargo.toml files (only the [package] version)
    # Excludes plugins/ which has independent versioning starting at 0.0.1
    for crate_toml in $(find "$ROOT_DIR/crates" -name "Cargo.toml"); do
        # Replace only the first occurrence of version = "..." under [package]
        sed -i -E "0,/^version = \"[^\"]+\"/ s/^version = \"[^\"]+\"/version = \"$target_ver\"/" "$crate_toml"
        echo "  [✓] Updated $crate_toml"
    done


    # 4. Update documentation references in docs/causm_index.md
    if [ -f "$ROOT_DIR/docs/causm_index.md" ]; then
        sed -i -E "s/v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?/v$target_ver/g" "$ROOT_DIR/docs/causm_index.md"
        echo "  [✓] Updated docs/causm_index.md"
    fi

    # 5. Update README.md header if it references current version
    if [ -f "$ROOT_DIR/README.md" ]; then
        sed -i -E "s/v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?/v$target_ver/g" "$ROOT_DIR/README.md"
        echo "  [✓] Updated README.md"
    fi

    echo "==> Successfully synchronized all Causm components to v$target_ver"
}

parse_and_bump() {
    local bump_type="$1"
    local raw_ver="$CURRENT_VERSION"

    # Regex for SemVer with optional pre-release (e.g., 0.1.0-alpha.1)
    local semver_regex='^([0-9]+)\.([0-9]+)\.([0-9]+)(-([a-zA-Z]+)\.([0-9]+))?$'
    if [[ ! $raw_ver =~ $semver_regex ]]; then
        echo "Error: Current version '$raw_ver' does not match SemVer format X.Y.Z[-prerelease.N]" >&2
        exit 1
    fi

    local major="${BASH_REMATCH[1]}"
    local minor="${BASH_REMATCH[2]}"
    local patch="${BASH_REMATCH[3]}"
    local pre_tag="${BASH_REMATCH[5]:-}"
    local pre_num="${BASH_REMATCH[6]:-0}"

    local new_ver=""

    case "$bump_type" in
        major)
            new_ver="$((major + 1)).0.0"
            ;;
        minor)
            new_ver="${major}.$((minor + 1)).0"
            ;;
        patch)
            new_ver="${major}.${minor}.$((patch + 1))"
            ;;
        alpha)
            if [ "$pre_tag" = "alpha" ]; then
                new_ver="${major}.${minor}.${patch}-alpha.$((pre_num + 1))"
            else
                new_ver="${major}.${minor}.${patch}-alpha.1"
            fi
            ;;
        beta)
            if [ "$pre_tag" = "beta" ]; then
                new_ver="${major}.${minor}.${patch}-beta.$((pre_num + 1))"
            else
                new_ver="${major}.${minor}.${patch}-beta.1"
            fi
            ;;
        rc)
            if [ "$pre_tag" = "rc" ]; then
                new_ver="${major}.${minor}.${patch}-rc.$((pre_num + 1))"
            else
                new_ver="${major}.${minor}.${patch}-rc.1"
            fi
            ;;
        release)
            new_ver="${major}.${minor}.${patch}"
            ;;
        *)
            echo "Error: Unknown bump type '$bump_type'. Valid types: major, minor, patch, alpha, beta, rc, release" >&2
            exit 1
            ;;
    esac

    update_files "$new_ver"
}

if [ $# -lt 1 ]; then
    usage
fi

COMMAND="$1"

case "$COMMAND" in
    current)
        echo "Current Causm Version: v$CURRENT_VERSION"
        ;;
    set)
        if [ $# -lt 2 ]; then
            echo "Error: Missing target version" >&2
            usage
        fi
        TARGET_VER="${2#v}" # Strip leading 'v' if provided
        update_files "$TARGET_VER"
        ;;
    bump)
        if [ $# -lt 2 ]; then
            echo "Error: Missing bump type (major|minor|patch|alpha|beta|rc|release)" >&2
            usage
        fi
        parse_and_bump "$2"
        ;;
    branch)
        create_branch "${2:-release}"
        ;;
    *)
        usage
        ;;
esac
