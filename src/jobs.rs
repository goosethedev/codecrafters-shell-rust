use std::process::{Child, Command, Stdio};

use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobStatus {
    Running,
    Done(i32),
}

pub struct Job {
    id: u32,
    command: String,
    child: Child,
    status: JobStatus,
}

#[derive(Default)]
pub struct JobTable {
    jobs: IndexMap<u32, Job>,
}

impl JobTable {
    pub fn spawn_background(
        &mut self,
        instruction: crate::parser::Instruction,
    ) -> std::io::Result<u32> {
        let command = instruction.prettify();
        let child = Command::new(instruction.cmd)
            .args(instruction.args)
            .stdout(match instruction.stdout {
                Some(f) => Stdio::from(f),
                None => Stdio::inherit(),
            })
            .stderr(match instruction.stderr {
                Some(f) => Stdio::from(f),
                None => Stdio::inherit(),
            })
            .spawn()?;

        let id = self.next_id();
        let pid = child.id();
        self.jobs.insert(
            id,
            Job {
                id,
                command,
                child,
                status: JobStatus::Running,
            },
        );
        println!("[{id}] {pid}");
        Ok(id)
    }

    pub fn poll(&mut self) {
        for job in self.jobs.values_mut() {
            if job.status == JobStatus::Running
                && let Ok(Some(status)) = job.child.try_wait()
            {
                job.status = JobStatus::Done(status.code().unwrap_or(-1));
            }
        }
    }

    pub fn list_and_reap(&mut self, only_done: bool) {
        let mut finished = vec![];
        for (i, job) in self.jobs.values().enumerate() {
            let sign = match self.jobs.len() - i {
                1 => "+",
                2 => "-",
                _ => " ",
            };
            let (status, amper) = match job.status {
                JobStatus::Running => ("Running", " &"),
                JobStatus::Done(_) => {
                    finished.push(job.id);
                    ("Done", "")
                }
            };

            if !(only_done && status != "Done") {
                println!(
                    "[{}]{}  {:<24}{}{}",
                    job.id, sign, status, job.command, amper
                );
            }
        }
        for job in finished {
            self.jobs.shift_remove(&job);
        }
    }

    // TODO: Optimize?
    fn next_id(&self) -> u32 {
        let mut id = 1u32;
        while self.jobs.contains_key(&id) {
            id += 1;
        }
        id
    }
}
