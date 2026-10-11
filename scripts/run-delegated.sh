#!/usr/bin/env bash
# Run only inside a systemd Delegate=yes service/scope owned by this user.
set -euo pipefail
relative=$(awk -F: '$1=="0" {print $3}' /proc/self/cgroup)
[[ -n "$relative" && "$relative" != / ]] || { echo 'Refusing cgroup root' >&2; exit 1; }
root="/sys/fs/cgroup$relative"
[[ -w "$root/cgroup.subtree_control" && -w "$root/cgroup.procs" ]] || { echo 'A delegated cgroup v2 service is required' >&2; exit 1; }
mkdir -p "$root/supervisor" "$root/workers"
printf '%s' "$$" > "$root/supervisor/cgroup.procs"
[[ -z "$(cat "$root/cgroup.procs")" ]] || { echo 'Other processes remain in delegated root; refusing controller changes' >&2; exit 1; }
printf '+memory +pids' > "$root/cgroup.subtree_control"
printf '+memory +pids' > "$root/workers/cgroup.subtree_control"
export DELTA_CGROUP_ROOT="$root/workers"
exec "$@"
