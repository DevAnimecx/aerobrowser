use anyhow::{bail, Result};
// Only the Landlock path loop calls with_context, and that loop is Linux-only,
// so the trait import is gated to match rather than left unused elsewhere.
#[cfg(target_os = "linux")]
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub read: Vec<PathBuf>,
    #[serde(default)]
    pub write: Vec<PathBuf>,
    #[serde(default = "all")]
    pub network: String,
    #[serde(default)]
    pub allow_child_processes: bool,
    #[serde(default = "cpu")]
    pub cpu_seconds: u64,
    #[serde(default = "fds")]
    pub max_open_files: u64,
}
fn all() -> String {
    "all".into()
}
fn cpu() -> u64 {
    300
}
fn fds() -> u64 {
    256
}
impl Profile {
    pub fn built_in(name: &str) -> Result<Self> {
        if ![
            "scrape",
            "offline",
            "web-dev",
            "test",
            "base",
            "untrusted",
            "full",
        ]
        .contains(&name)
        {
            bail!("Unknown profile {name}");
        }
        // Loopback/domain policies require an egress broker; do not silently downgrade.
        if ["base", "web-dev", "test"].contains(&name) {
            bail!("Profile {name} requires kernel-mediated loopback egress, not available in this release. Choose scrape (unrestricted egress) or offline (deny outbound connections) explicitly.");
        }
        Ok(Self {
            name: name.into(),
            read: vec![],
            write: vec![],
            network: if ["offline", "untrusted"].contains(&name) {
                "none"
            } else {
                "all"
            }
            .into(),
            allow_child_processes: false,
            cpu_seconds: cpu(),
            max_open_files: fds(),
        })
    }
    pub fn load(name: &str) -> Result<Self> {
        let p = if name.ends_with(".json") {
            serde_json::from_slice(&std::fs::read(name)?)?
        } else {
            Self::built_in(name)?
        };
        if !["all", "none"].contains(&p.network.as_str()) {
            bail!("Unsupported network policy; refusing to weaken it");
        }
        if p.cpu_seconds == 0 || p.max_open_files < 16 {
            bail!("Invalid limits");
        }
        Ok(p)
    }
}
pub fn probe() -> serde_json::Value {
    // Each platform block is the tail of the function rather than an early
    // `return`. Clippy's needless_return only fires on the branch a given
    // target actually compiles, which is why the Linux build was the one
    // failing and a Windows checkout was clean.
    #[cfg(target_os = "linux")]
    unsafe {
        let abi = libc::syscall(444, std::ptr::null::<u8>(), 0, 1);
        serde_json::json!({"platform":"linux","landlock_abi":if abi>=0{Some(abi)}else{None},"supported":abi>=2,"required_abi":2,"abi2_compatibility":"truncate/ftruncate syscalls denied by seccomp","seccomp":true})
    }
    #[cfg(target_os = "macos")]
    {
        serde_json::json!({"platform":"macos","seatbelt_available":std::path::Path::new("/usr/bin/sandbox-exec").exists(),"validated":false})
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Say why, so `delta doctor` and `server_status` report something useful
        // instead of a bare false.
        serde_json::json!({
            "supported": false,
            "platform": std::env::consts::OS,
            "detail": "No kernel isolation is implemented on this platform; use an external browser and treat containment as its responsibility",
        })
    }
}
#[cfg(target_os = "linux")]
pub fn apply(profile: &Profile, executable: &std::path::Path) -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    #[repr(C)]
    struct Ruleset {
        access_fs: u64,
    }
    #[repr(C, packed)]
    struct PathRule {
        allowed: u64,
        parent: i32,
    }
    const READ: u64 = (1 << 2) | (1 << 3);
    const EXEC: u64 = 1;
    const ALL: u64 = (1 << 15) - 1;
    unsafe {
        let abi = libc::syscall(444, std::ptr::null::<u8>(), 0, 1);
        if abi < 2 {
            bail!("Landlock ABI >=2 required; fail-closed (detected {abi})");
        }
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            bail!("no_new_privs: {}", std::io::Error::last_os_error());
        }
        let supported = if abi >= 3 { ALL } else { (1 << 14) - 1 };
        let ruleset = Ruleset {
            access_fs: supported,
        };
        let fd = libc::syscall(444, &ruleset, std::mem::size_of::<Ruleset>(), 0) as i32;
        if fd < 0 {
            bail!("Landlock create: {}", std::io::Error::last_os_error());
        }
        let result = (|| -> Result<()> {
            let mut grants: Vec<(PathBuf, u64)> =
                profile.read.iter().map(|p| (p.clone(), READ)).collect();
            grants.extend(profile.write.iter().map(|p| (p.clone(), ALL & !EXEC)));
            grants.push((executable.to_path_buf(), READ | EXEC));
            for loader in ["/lib64/ld-linux-x86-64.so.2", "/lib/ld-linux-aarch64.so.1"] {
                let p = PathBuf::from(loader);
                if p.exists() {
                    grants.push((p, READ | EXEC));
                }
            }
            for (path, mut access) in grants {
                access &= supported;
                let path = path
                    .canonicalize()
                    .with_context(|| format!("Sandbox path {}", path.display()))?;
                if !path.is_dir() {
                    access &= (1 << 0) | (1 << 1) | (1 << 2) | (1 << 14);
                }
                let c = CString::new(path.as_os_str().as_bytes())?;
                let pfd = libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC);
                if pfd < 0 {
                    bail!("Open sandbox path: {}", std::io::Error::last_os_error());
                }
                let rule = PathRule {
                    allowed: access,
                    parent: pfd,
                };
                let rc = libc::syscall(445, fd, 1, &rule, 0);
                libc::close(pfd);
                if rc != 0 {
                    bail!("Landlock add rule: {}", std::io::Error::last_os_error());
                }
            }
            if libc::syscall(446, fd, 0) != 0 {
                bail!("Landlock restrict: {}", std::io::Error::last_os_error());
            }
            Ok(())
        })();
        libc::close(fd);
        result?;
        let cpu = libc::rlimit {
            rlim_cur: profile.cpu_seconds,
            rlim_max: profile.cpu_seconds,
        };
        let files = libc::rlimit {
            rlim_cur: profile.max_open_files,
            rlim_max: profile.max_open_files,
        };
        if libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0
            || libc::setrlimit(libc::RLIMIT_NOFILE, &files) != 0
        {
            bail!("Failed to apply resource limits");
        }
        // Seccomp applies to this single exec'ing thread and its descendants.
        let stmt = |code: u16, k: u32| libc::sock_filter {
            code,
            jt: 0,
            jf: 0,
            k,
        };
        let jump = |k: u32, jt: u8, jf: u8| libc::sock_filter {
            code: 0x15,
            jt,
            jf,
            k,
        };
        #[cfg(target_arch = "x86_64")]
        let arch = 0xc000003e;
        #[cfg(target_arch = "aarch64")]
        let arch = 0xc00000b7;
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        compile_error!("Sandbox supports x86_64/aarch64 only");
        let mut bpf = vec![
            stmt(0x20, 4),
            jump(arch, 1, 0),
            stmt(0x06, 0x80000000),
            stmt(0x20, 0),
        ];
        let denied = [
            libc::SYS_ptrace,
            libc::SYS_mount,
            libc::SYS_umount2,
            libc::SYS_pivot_root,
            libc::SYS_bpf,
            libc::SYS_keyctl,
            libc::SYS_add_key,
            libc::SYS_request_key,
            libc::SYS_reboot,
            libc::SYS_kexec_load,
            libc::SYS_process_vm_readv,
            libc::SYS_process_vm_writev,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_userfaultfd,
            libc::SYS_io_uring_setup,
            libc::SYS_pidfd_open,
            libc::SYS_pidfd_getfd,
            libc::SYS_pidfd_send_signal,
            libc::SYS_perf_event_open,
            libc::SYS_setsid,
            libc::SYS_setpgid,
        ];
        for nr in denied {
            bpf.push(jump(nr as u32, 0, 1));
            bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
        }
        if abi < 3 {
            // ABI2 does not mediate truncation. open(O_RDONLY|O_TRUNC) is otherwise a real bypass.
            // Deny that combination and force openat2 to a filterable openat fallback.
            bpf.push(jump(libc::SYS_openat2 as u32, 0, 1));
            bpf.push(stmt(0x06, 0x00050000 | libc::ENOSYS as u32));
            let mut opens = vec![(libc::SYS_openat, 32)];
            #[cfg(target_arch = "x86_64")]
            opens.push((libc::SYS_open, 24));
            for (nr, offset) in opens {
                bpf.push(jump(nr as u32, 0, 6));
                bpf.push(stmt(0x20, offset));
                bpf.push(libc::sock_filter {
                    code: 0x45,
                    jt: 0,
                    jf: 3,
                    k: libc::O_TRUNC as u32,
                });
                bpf.push(stmt(0x54, libc::O_ACCMODE as u32));
                bpf.push(jump(libc::O_RDONLY as u32, 0, 1));
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
                bpf.push(stmt(0x20, 0));
            }
            for nr in [libc::SYS_truncate, libc::SYS_ftruncate] {
                bpf.push(jump(nr as u32, 0, 1));
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            }
        }
        #[cfg(target_arch = "x86_64")]
        {
            bpf.push(libc::sock_filter {
                code: 0x45,
                jt: 0,
                jf: 1,
                k: 0x40000000,
            });
            bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
        }
        if profile.network == "none" {
            bpf.push(jump(libc::SYS_connect as u32, 0, 1));
            bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            // Only stream sockets: no UDP/raw socket egress. Accepted CDP TCP replies remain usable.
            bpf.push(jump(libc::SYS_socket as u32, 0, 5));
            bpf.push(stmt(0x20, 24));
            bpf.push(stmt(0x54, 0xf));
            bpf.push(jump(libc::SOCK_STREAM as u32, 1, 0));
            bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            bpf.push(stmt(0x20, 0));
            // sendto with a destination can initiate TCP Fast Open without connect.
            bpf.push(jump(libc::SYS_sendto as u32, 0, 7));
            for offset in [48, 52] {
                bpf.push(stmt(0x20, offset));
                bpf.push(jump(0, 1, 0));
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            }
            bpf.push(stmt(0x20, 0));
            for (nr, offset) in [
                (libc::SYS_sendto, 40),
                (libc::SYS_sendmsg, 32),
                (libc::SYS_sendmmsg, 40),
            ] {
                bpf.push(jump(nr as u32, 0, 4));
                bpf.push(stmt(0x20, offset));
                bpf.push(libc::sock_filter {
                    code: 0x45,
                    jt: 0,
                    jf: 1,
                    k: libc::MSG_FASTOPEN as u32,
                });
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
                bpf.push(stmt(0x20, 0));
            }
        }
        if !profile.allow_child_processes {
            let pid = libc::getpid() as u32;
            // A worker must not signal peer sessions or manipulate their limits/affinity.
            for nr in [
                libc::SYS_kill,
                libc::SYS_tgkill,
                libc::SYS_prlimit64,
                libc::SYS_sched_setaffinity,
            ] {
                bpf.push(jump(nr as u32, 0, 5));
                bpf.push(stmt(0x20, 16));
                bpf.push(jump(pid, 2, 0));
                // pid=0 is safe for prlimit/affinity, but kill(0) signals the whole group.
                bpf.push(jump(
                    if nr == libc::SYS_prlimit64 || nr == libc::SYS_sched_setaffinity {
                        0
                    } else {
                        pid
                    },
                    1,
                    0,
                ));
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
                bpf.push(stmt(0x20, 0));
            }
            for nr in [libc::SYS_tkill, libc::SYS_setpriority] {
                bpf.push(jump(nr as u32, 0, 1));
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            }

            #[cfg(target_arch = "x86_64")]
            for nr in [libc::SYS_fork, libc::SYS_vfork] {
                bpf.push(jump(nr as u32, 0, 1));
                bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            }
            // Return ENOSYS for clone3 so libc falls back to filterable clone flags.
            bpf.push(jump(libc::SYS_clone3 as u32, 0, 1));
            bpf.push(stmt(0x06, 0x00050000 | libc::ENOSYS as u32));
            bpf.push(jump(libc::SYS_clone as u32, 0, 4));
            bpf.push(stmt(0x20, 16));
            bpf.push(libc::sock_filter {
                code: 0x45,
                jt: 1,
                jf: 0,
                k: libc::CLONE_THREAD as u32,
            });
            bpf.push(stmt(0x06, 0x00050000 | libc::EPERM as u32));
            bpf.push(stmt(0x20, 0));
        }
        bpf.push(stmt(0x06, 0x7fff0000));
        let prog = libc::sock_fprog {
            len: bpf.len() as u16,
            filter: bpf.as_mut_ptr(),
        };
        if libc::prctl(libc::PR_SET_SECCOMP, 2, &prog) != 0 {
            bail!("seccomp: {}", std::io::Error::last_os_error());
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn apply(_: &Profile, _: &std::path::Path) -> Result<()> {
    bail!("Direct sandbox application supported only on Linux; macOS uses the Seatbelt launcher")
}
