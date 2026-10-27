//! 每個設定目錄只保留一個編輯器；以檔案鎖選主，再經本機連線轉交開檔要求。

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    pid: u32,
    token: String,
}

#[derive(Serialize, Deserialize)]
struct Request {
    token: String,
    files: Vec<PathBuf>,
}

pub struct Instance {
    // 鎖檔不得刪除，否則 Unix 上後來的程序可能鎖到不同 inode 而同時成為主程序。
    _lock: File,
    endpoint_path: PathBuf,
    incoming: mpsc::Receiver<Vec<PathBuf>>,
    context: Arc<Mutex<Option<egui::Context>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Instance {
    /// None 表示要求已交給既有程序，呼叫端應直接結束，不初始化 GUI 或讀寫工作階段。
    pub fn start(directory: &Path, files: Vec<PathBuf>) -> io::Result<Option<Self>> {
        fs::create_dir_all(directory)?;
        let cwd = std::env::current_dir()?;
        let files: Vec<_> = files
            .into_iter()
            .map(|p| if p.is_absolute() { p } else { cwd.join(p) })
            .collect();
        let endpoint_path = directory.join("instance.json");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("instance.lock"))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match lock.try_lock() {
                Ok(()) => return Self::listen(lock, endpoint_path).map(Some),
                Err(TryLockError::Error(e)) => return Err(e),
                Err(TryLockError::WouldBlock) => {}
            }
            // 主程序可能仍在啟動，或正好退出；失敗時重新選主，不另開第二個視窗。
            if Self::forward(&endpoint_path, &files).is_ok() {
                return Ok(None);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "無法將開檔要求交給執行中的 MadEdit",
                ));
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn listen(lock: File, endpoint_path: PathBuf) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = Endpoint {
            port: listener.local_addr()?.port(),
            pid: std::process::id(),
            token: format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
        };
        fs::write(&endpoint_path, serde_json::to_vec(&endpoint)?)?;
        let (sender, incoming) = mpsc::channel();
        let context = Arc::new(Mutex::new(None::<egui::Context>));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_context = context.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let result = (|| -> io::Result<()> {
                                stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                                stream.set_write_timeout(Some(Duration::from_secs(1)))?;
                                let request: Request = receive(&mut stream)?;
                                if request.token != endpoint.token {
                                    return Err(io::Error::new(
                                        io::ErrorKind::PermissionDenied,
                                        "無效的開檔要求",
                                    ));
                                }
                                sender.send(request.files).map_err(io::Error::other)?;
                                if let Some(ctx) = worker_context.lock().unwrap().as_ref() {
                                    ctx.request_repaint();
                                }
                                stream.write_all(&[1])
                            })();
                            let _ = result;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20))
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Self {
            _lock: lock,
            endpoint_path,
            incoming,
            context,
            stop,
            worker: Some(worker),
        })
    }

    fn forward(path: &Path, files: &[PathBuf]) -> io::Result<()> {
        let endpoint: Endpoint = serde_json::from_slice(&fs::read(path)?)?;
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, endpoint.port);
        let mut stream = TcpStream::connect_timeout(&address.into(), Duration::from_millis(250))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        allow_foreground(endpoint.pid);
        let payload = serde_json::to_vec(&Request {
            token: endpoint.token,
            files: files.to_vec(),
        })?;
        stream.write_all(&(payload.len() as u32).to_le_bytes())?;
        stream.write_all(&payload)?;
        let mut ack = [0];
        stream.read_exact(&mut ack)?;
        if ack != [1] {
            return Err(io::Error::other("MadEdit 未確認開檔要求"));
        }
        Ok(())
    }

    pub fn attach_context(&self, ctx: &egui::Context) {
        *self.context.lock().unwrap() = Some(ctx.clone());
        ctx.request_repaint();
    }

    pub fn requests(&self) -> Vec<Vec<PathBuf>> {
        self.incoming.try_iter().collect()
    }
}

fn receive(stream: &mut TcpStream) -> io::Result<Request> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > 1024 * 1024 {
        return Err(io::Error::other("開檔要求過大"));
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

#[cfg(windows)]
fn allow_foreground(pid: u32) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
    }
    // 由使用者啟動的後續程序授權主程序取得前景，避免 Windows 只閃爍工作列。
    unsafe {
        AllowSetForegroundWindow(pid);
    }
}

#[cfg(not(windows))]
fn allow_foreground(_pid: u32) {}

impl Drop for Instance {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_file(&self.endpoint_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_files_activation_and_recovers_after_exit() {
        let dir = std::env::temp_dir().join(format!(
            "madedit-instance-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // 遺留端點不能阻止重新啟動；作業系統鎖才是選主的依據。
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("instance.json"), "stale").unwrap();
        let primary = Instance::start(&dir, vec![]).unwrap().unwrap();
        let files = vec![PathBuf::from("中文 空白.txt"), dir.join("missing.txt")];
        assert!(Instance::start(&dir, files.clone()).unwrap().is_none());
        assert_eq!(
            primary.requests(),
            vec![vec![
                std::env::current_dir().unwrap().join(&files[0]),
                files[1].clone()
            ]]
        );
        assert!(Instance::start(&dir, vec![]).unwrap().is_none());
        assert_eq!(primary.requests(), vec![Vec::<PathBuf>::new()]);
        drop(primary);
        let replacement = Instance::start(&dir, vec![]).unwrap().unwrap();
        drop(replacement);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_launches_choose_one_primary() {
        let dir = std::env::temp_dir().join(format!(
            "madedit-race-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let barrier = Arc::new(std::sync::Barrier::new(6));
        let workers: Vec<_> = (0..6)
            .map(|_| {
                let dir = dir.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    Instance::start(&dir, vec![]).unwrap()
                })
            })
            .collect();
        let instances: Vec<_> = workers
            .into_iter()
            .filter_map(|w| w.join().unwrap())
            .collect();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].requests().len(), 5);
        drop(instances);
        fs::remove_dir_all(dir).unwrap();
    }
}
