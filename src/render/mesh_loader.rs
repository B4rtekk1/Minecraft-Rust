use crate::core::vertex::Vertex;
use crate::world::World;
use crossbeam_channel::{Receiver, Sender, bounded};
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

pub struct MeshRequest {
    pub cx: i32,
    pub cz: i32,
    pub sy: i32,
}

pub struct MeshResult {
    pub cx: i32,
    pub cz: i32,
    pub sy: i32,
    pub terrain: (Vec<Vertex>, Vec<u32>),
    pub water: (Vec<Vertex>, Vec<u32>),
}

pub struct MeshLoader {
    request_tx: Sender<MeshRequest>,
    result_rx: Receiver<MeshResult>,
    pending: HashSet<(i32, i32, i32)>,
}

impl MeshLoader {
    pub fn new(world: Arc<parking_lot::RwLock<World>>, worker_count: usize) -> Self {
        let (request_tx, request_rx) = bounded::<MeshRequest>(256);
        let (result_tx, result_rx) = bounded::<MeshResult>(256);

        for i in 0..worker_count {
            let rx = request_rx.clone();
            let tx = result_tx.clone();
            let world = Arc::clone(&world);

            thread::Builder::new()
                .name(format!("mesh-worker-{}", i))
                .spawn(move || {    
                    while let Ok(req) = rx.recv() {
                        let meshes = {
                            let world_read = world.read();
                            world_read.build_subchunk_mesh(req.cx, req.cz, req.sy)
                        };

                        if tx
                            .send(MeshResult {
                                cx: req.cx,
                                cz: req.cz,
                                sy: req.sy,
                                terrain: meshes.0,
                                water: meshes.1,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .expect("Failed to spawn mesh worker");
        }

        Self {
            request_tx,
            result_rx,
            pending: HashSet::new(),
        }
    }

    pub fn request_mesh(&mut self, cx: i32, cz: i32, sy: i32) {
        let key = (cx, cz, sy);
        if self.pending.contains(&key) {
            return; // Already queued, skip
        }
        match self.request_tx.try_send(MeshRequest { cx, cz, sy }) {
            Ok(_) => {
                self.pending.insert(key);
            }
            Err(_) => {
                // Queue full – will retry next frame (mesh_dirty stays true)
            }
        }
    }

    pub fn poll_result(&mut self) -> Option<MeshResult> {
        match self.result_rx.try_recv() {
            Ok(result) => {
                self.pending.remove(&(result.cx, result.cz, result.sy));
                Some(result)
            }
            Err(_) => None,
        }
    }

    /// Returns true if this subchunk is already queued for mesh building.
    pub fn is_pending(&self, cx: i32, cz: i32, sy: i32) -> bool {
        self.pending.contains(&(cx, cz, sy))
    }
}
