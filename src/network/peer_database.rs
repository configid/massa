use chrono::{DateTime, Utc};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::net::IpAddr;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{delay_for, Duration};

type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Debug)]
pub enum PeerStatus {
    Idle,
    OutConnecting,
    OutHandshaking,
    OutConnected,
    InHandshaking,
    InConnected,
    Banned,
}
impl Default for PeerStatus {
    fn default() -> Self {
        PeerStatus::Idle
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct PeerInfo {
    pub ip: IpAddr,
    pub status: PeerStatus,
    pub last_connection: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
}

pub struct PeerDatabase {
    pub peers: HashMap<IpAddr, PeerInfo>,
    peers_filename: String,
    saver_join_handle: JoinHandle<()>,
    saver_watch_tx: watch::Sender<Option<HashMap<IpAddr, PeerInfo>>>,
}

async fn load_peers(file_name: &String) -> BoxResult<HashMap<IpAddr, PeerInfo>> {
    let result =
        serde_json::from_str::<Vec<PeerInfo>>(&tokio::fs::read_to_string(file_name).await?)?
            .iter()
            .map(|p| match p.status {
                PeerStatus::Idle | PeerStatus::Banned => Ok((p.ip, *p)),
                _ => Err("invalid peer status in peers file"),
            })
            .collect::<Result<HashMap<IpAddr, PeerInfo>, _>>()?;
    if result.is_empty() {
        return Err("known peers file is empty".into());
    }
    Ok(result)
}

async fn dump_peers(peers: &HashMap<IpAddr, PeerInfo>, file_name: &String) -> BoxResult<()> {
    let peer_vec: Vec<PeerInfo> = peers
        .values()
        .map(|&p| PeerInfo {
            status: match p.status {
                PeerStatus::Banned => PeerStatus::Banned,
                _ => PeerStatus::Idle,
            },
            ..p
        })
        .collect();
    tokio::fs::write(file_name, serde_json::to_string_pretty(&peer_vec)?).await?;
    Ok(())
}

impl PeerDatabase {
    pub async fn load(
        peers_filename: String,
        peer_file_dump_interval_seconds: f32,
    ) -> BoxResult<Self> {
        // load from file
        let peers = load_peers(&peers_filename).await?;

        // setup saver
        let peers_filename_copy = peers_filename.clone();
        let (saver_watch_tx, mut saver_watch_rx) = watch::channel(Some(peers.clone()));
        let saver_join_handle = tokio::spawn(async move {
            let mut delay = delay_for(Duration::from_secs_f32(peer_file_dump_interval_seconds));
            let mut last_value: Option<HashMap<IpAddr, PeerInfo>> = None;
            loop {
                tokio::select! {
                    opt_opt_p = saver_watch_rx.recv() => match opt_opt_p {
                        Some(Some(op)) => {
                            if last_value.is_none() {

                            }
                            last_value = Some(op);
                        },
                        _ => break
                    },
                    _ = &mut delay => {
                        if let Some(ref p) = last_value {
                            if let Err(e) = dump_peers(&p, &peers_filename_copy).await {
                                warn!("could not dump peers to file: {}", e);
                                delay = delay_for(Duration::from_secs_f32(peer_file_dump_interval_seconds));
                                continue;
                            }
                            last_value = None;
                        }
                    }
                }
            }
            if let Some(p) = last_value {
                if let Err(e) = dump_peers(&p, &peers_filename_copy).await {
                    warn!("could not dump peers to file: {}", e);
                }
            }
        });

        // return struct
        Ok(PeerDatabase {
            peers,
            peers_filename,
            saver_join_handle,
            saver_watch_tx,
        })
    }

    pub fn save(&self) {
        if self
            .saver_watch_tx
            .broadcast(Some(self.peers.clone()))
            .is_err()
        {
            unreachable!("saver task disappeared");
        }
    }

    pub async fn stop(self) {
        let _ = self.saver_watch_tx.broadcast(None);
        let _ = self.saver_join_handle.await;
    }

    pub fn count_peers_with_status(&self, status: PeerStatus) -> usize {
        self.peers.values().filter(|&v| v.status == status).count()
    }

    pub fn cleanup(&mut self, max_known_nodes: usize) {
        // bookkeeping: drop old nodes etc...
        /* TODO bookeeping
            removes too old etc... if too many, drop randomly

            also remove too old banned nodes
        */
    }

    pub fn get_connector_candidate_ips(&self) -> HashSet<IpAddr> {
        /*
            TODO:
                missing_out_connections = max(0, target_out_connections - count_peers_with_status(outconnected)) warning: usize is unsigned !
                connection_slots_available = max(0, max_out_conn_attempts - count_peers_with_status(outconnecting) - count_peers_with_status(outhandshaking)) warning: usize is unsigned !
                n_to_try = min(missing_out_connections, connection_slots_available)
                => choose up to n_to_try candidates based on:
                    most_ancient failure (None = most ancient)
        */
        HashSet::new() //TODO compute
    }
}
