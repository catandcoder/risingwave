// Copyright 2022 Singularity Data
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};

use super::{ExecuteContext, Task};
use crate::util::{get_program_args, get_program_env_cmd, get_program_name};
use crate::ComputeNodeConfig;

pub struct ComputeNodeService {
    config: ComputeNodeConfig,
}

impl ComputeNodeService {
    pub fn new(config: ComputeNodeConfig) -> Result<Self> {
        Ok(Self { config })
    }

    fn compute_node(&self) -> Result<Command> {
        let prefix_bin = env::var("PREFIX_BIN")?;

        if let Ok(x) = env::var("ENABLE_ALL_IN_ONE") && x == "true" {
            Ok(Command::new(Path::new(&prefix_bin).join("risingwave").join("compute-node")))
        } else {
            Ok(Command::new(Path::new(&prefix_bin).join("compute-node")))
        }
    }

    /// Apply command args accroding to config
    pub fn apply_command_args(cmd: &mut Command, config: &ComputeNodeConfig) -> Result<()> {
        cmd.arg("--host")
            .arg(format!("{}:{}", config.address, config.port))
            .arg("--prometheus-listener-addr")
            .arg(format!(
                "{}:{}",
                config.exporter_address, config.exporter_port
            ))
            .arg("--metrics-level")
            .arg("1");

        let provide_jaeger = config.provide_jaeger.as_ref().unwrap();
        match provide_jaeger.len() {
            0 => {}
            1 => {
                cmd.arg("--enable-jaeger-tracing");
            }
            other_size => {
                return Err(anyhow!(
                    "{} Jaeger instance found in config, but only 1 is needed",
                    other_size
                ))
            }
        }

        let provide_minio = config.provide_minio.as_ref().unwrap();
        let provide_aws_s3 = config.provide_aws_s3.as_ref().unwrap();
        let provide_compute_node = config.provide_compute_node.as_ref().unwrap();

        let is_shared_backend = match (
            config.enable_in_memory_kv_state_backend,
            provide_minio.as_slice(),
            provide_aws_s3.as_slice(),
        ) {
            (true, [], []) => {
                cmd.arg("--state-store").arg("in-memory");
                false
            }
            (true, _, _) => {
                return Err(anyhow!(
                "When `enable_in_memory_kv_state_backend` is enabled, no minio and aws-s3 should be provided.",
            ))
            }
            (false, [], []) => {
                cmd.arg("--state-store").arg("hummock+memory");
                false
            }
            (false, [minio], []) => {
                cmd.arg("--state-store").arg(format!(
                    "hummock+minio://{hummock_user}:{hummock_password}@{minio_addr}:{minio_port}/{hummock_bucket}",
                    hummock_user = minio.hummock_user,
                    hummock_password = minio.hummock_password,
                    hummock_bucket = minio.hummock_bucket,
                    minio_addr = minio.address,
                    minio_port = minio.port,
                ));
                true
            }
            (false, [], [aws_s3]) => {
                cmd.arg("--state-store").arg(format!(
                    "hummock+s3://{}", aws_s3.bucket
                ));
                true
            }
            (false, other_minio, other_s3) => {
                return Err(anyhow!(
                    "{} minio and {} s3 instance found in config, but only 1 is needed",
                    other_minio.len(), other_s3.len()
                ))
            }
        };

        if provide_compute_node.len() > 1 && !is_shared_backend {
            return Err(anyhow!(
                "should use a shared backend (e.g. MinIO) for multiple compute-node configuration. Consider adding `use: minio` in risedev config."
            ));
        }

        let provide_meta_node = config.provide_meta_node.as_ref().unwrap();
        match provide_meta_node.as_slice() {
            [] => {
                return Err(anyhow!(
                    "Cannot start node: no meta node found in this configuration."
                ));
            }
            [meta_node] => {
                cmd.arg("--meta-address")
                    .arg(format!("http://{}:{}", meta_node.address, meta_node.port));
            }
            other_meta_nodes => {
                return Err(anyhow!(
                    "Cannot start node: {} meta nodes found in this configuration, but only 1 is needed.",
                    other_meta_nodes.len()
                ));
            }
        };

        let provide_compactor = config.provide_compactor.as_ref().unwrap();
        if is_shared_backend && provide_compactor.is_empty() {
            return Err(anyhow!(
                "When minio or aws-s3 is enabled, at least one compactor is required. Consider adding `use: compactor` in risedev config."
            ));
        }

        Ok(())
    }
}

impl Task for ComputeNodeService {
    fn execute(&mut self, ctx: &mut ExecuteContext<impl std::io::Write>) -> anyhow::Result<()> {
        ctx.service(self);
        ctx.pb.set_message("starting...");

        let prefix_config = env::var("PREFIX_CONFIG")?;

        let mut cmd = self.compute_node()?;

        cmd.env("RUST_BACKTRACE", "1");
        cmd.arg("--config-path")
            .arg(Path::new(&prefix_config).join("risingwave.toml"));
        Self::apply_command_args(&mut cmd, &self.config)?;

        if !self.config.user_managed {
            ctx.run_command(ctx.tmux_run(cmd)?)?;
            ctx.pb.set_message("started");
        } else {
            ctx.pb.set_message("user managed");
            writeln!(
                &mut ctx.log,
                "Please use the following parameters to start the compute node:\n{}\n{} {}\n\n",
                get_program_env_cmd(&cmd),
                get_program_name(&cmd),
                get_program_args(&cmd)
            )?;
        }

        Ok(())
    }

    fn id(&self) -> String {
        self.config.id.clone()
    }
}
