use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore, mpsc};

use crate::classes::job_handler::{HandlerConfig, JobHandler};
use crate::classes::models::job::Job;
use crate::classes::models::ts;

pub struct JobQueue {
    handler_config: HandlerConfig,
    sender: mpsc::UnboundedSender<Job>,
    active_jobs: Arc<RwLock<Vec<Arc<JobHandler>>>>,
}

impl JobQueue {
    pub async fn new(config: HandlerConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel::<Job>();

        let pending = Job::from_query(concat!(
            "SELECT id, imdb_id, title, poster_path, season, current_stage, last_log, preferences, progress, user_id, ",
            ts!("created_at"), " as created_at, ",
            ts!("updated_at"), " as updated_at ",
            "FROM jobs WHERE current_stage = 'pending'",
        ))
            .await
            .expect("Failed loading pending jobs");

        tracing::info!("Loaded {} pending job(s) from database", pending.len());

        for job in pending {
            sender.send(job).expect("Failed to enqueue pending job");
        }

        let queue = JobQueue {
            handler_config: config,
            sender,
            active_jobs: Arc::new(RwLock::new(Vec::new())),
        };

        let max_concurrent = queue.handler_config.jobs_config.max_concurrent;
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        queue.spawn_worker(receiver, semaphore);

        tracing::info!("Job queue started with max concurrency of {max_concurrent}");

        queue
    }

    pub async fn push(&self, job: Job) {
        job.save().await;
        self.sender.send(job).expect("Job queue channel closed");
    }

    fn spawn_worker(&self, mut receiver: mpsc::UnboundedReceiver<Job>, semaphore: Arc<Semaphore>) {
        let active_jobs = self.active_jobs.clone();
        let config = self.handler_config.clone();

        tokio::spawn(async move {
            while let Some(job) = receiver.recv().await {
                let permit = semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("Semaphore closed");
                let job_id = job.id;

                tracing::info!("Starting job {job_id}");

                let handler = Arc::new(JobHandler::new(job, config.clone()));

                active_jobs.write().await.push(handler.clone());

                tokio::spawn(async move {
                    handler.start().await;
                    drop(permit);
                    tracing::info!("Finished job {job_id}");
                });
            }

            tracing::debug!("Job queue channel closed, worker exiting");
        });
    }
}
