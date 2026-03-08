use std::cmp::Ordering;
use std::collections::HashSet;
use std::thread;

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};

use crate::core::chunk::Chunk;
use crate::world::generator::ChunkGenerator;

#[derive(Clone)]
pub struct ChunkGenRequest {
    pub cx: i32,
    pub cz: i32,
    pub priority: i32,
}

impl PartialEq for ChunkGenRequest {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for ChunkGenRequest {}

impl PartialOrd for ChunkGenRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkGenRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority)
    }
}

pub struct ChunkGenResult {
    pub cx: i32,
    pub cz: i32,
    pub chunk: Chunk,
}

pub struct ChunkLoader {
    request_tx: Sender<ChunkGenRequest>,
    result_rx: Receiver<ChunkGenResult>,
    pending: HashSet<(i32, i32)>,
    worker_count: usize,
}

impl ChunkLoader {
    pub fn new(seed: u32) -> Self {
        Self::with_worker_count(crate::constants::get_chunk_worker_count(), seed)
    }

    pub fn with_worker_count(num_workers: usize, seed: u32) -> Self {
        let (request_tx, request_rx) = bounded::<ChunkGenRequest>(256);
        let (result_tx, result_rx) = bounded::<ChunkGenResult>(256);

        for worker_id in 0..num_workers {
            let rx = request_rx.clone();
            let tx = result_tx.clone();
            let generator = ChunkGenerator::new(seed);

            thread::Builder::new()
                .name(format!("chunk-gen-{}", worker_id))
                .spawn(move || {
                    loop {
                        match rx.recv() {
                            Ok(req) => {
                                // Generate the chunk using
                                let chunk = generator.generate_chunk(req.cx, req.cz);

                                if tx
                                    .send(ChunkGenResult {
                                        cx: req.cx,
                                        cz: req.cz,
                                        chunk,
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                })
                .expect("Failed to spawn chunk generation worker");
        }

        ChunkLoader {
            request_tx,
            result_rx,
            pending: HashSet::new(),
            worker_count: num_workers,
        }
    }

    pub fn request_chunk(&mut self, cx: i32, cz: i32, priority: i32) {
        if self.pending.contains(&(cx, cz)) {
            return;
        }

        self.pending.insert((cx, cz));

        if self
            .request_tx
            .try_send(ChunkGenRequest { cx, cz, priority })
            .is_err()
        {
            self.pending.remove(&(cx, cz));
        }
    }

    pub fn request_chunks(&mut self, requests: &[(i32, i32, i32)]) {
        let mut sorted: Vec<_> = requests
            .iter()
            .filter(|(cx, cz, _)| !self.pending.contains(&(*cx, *cz)))
            .collect();
        sorted.sort_by_key(|(_, _, priority)| *priority);

        for (cx, cz, priority) in sorted {
            if self.pending.len() >= 256 {
                break;
            }
            self.pending.insert((*cx, *cz));
            if self
                .request_tx
                .try_send(ChunkGenRequest {
                    cx: *cx,
                    cz: *cz,
                    priority: *priority,
                })
                .is_err()
            {
                self.pending.remove(&(*cx, *cz));
            }
        }
    }

    pub fn is_pending(&self, cx: i32, cz: i32) -> bool {
        self.pending.contains(&(cx, cz))
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn poll_results(&mut self, max_results: usize) -> Vec<ChunkGenResult> {
        let mut results = Vec::with_capacity(max_results);

        for _ in 0..max_results {
            match self.result_rx.try_recv() {
                Ok(result) => {
                    self.pending.remove(&(result.cx, result.cz));
                    results.push(result);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        results
    }

    pub fn poll_all_results(&mut self) -> Vec<ChunkGenResult> {
        self.poll_results(64)
    }

    pub fn cancel(&mut self, cx: i32, cz: i32) {
        self.pending.remove(&(cx, cz));
    }

    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }

    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}
