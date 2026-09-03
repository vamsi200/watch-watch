#![allow(unused)]

use std::time::Duration;

use anyhow::Context;
use rdkafka::{
    ClientConfig,
    admin::{
        AdminClient, AdminOptions, NewPartitions, NewTopic, PartitionAssignment, ResourceSpecifier,
        TopicReplication,
    },
    client::{Client, DefaultClientContext},
    config::FromClientConfig,
    consumer::StreamConsumer,
    metadata::Metadata,
    producer::FutureProducer,
};

#[derive(Debug)]
pub struct ConsumerConfig {
    pub key_value: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct ProducerConfig {
    pub key_value: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct AdminKafkaConfig {
    pub bootstrap_servers: Vec<String>,
    pub consumer_config: Option<ConsumerConfig>,
    pub producer_config: Option<ProducerConfig>,
}

#[derive(Debug)]
pub struct AdminTopicConfig {
    pub name: String,
    pub partitions: i32,
    pub replication_factor: i32,
}

// TODO: add support for Variable Replication
#[derive(Debug)]
pub enum Replication {
    Fixed(i32),
    Variable(Vec<Vec<i32>>),
}

pub struct KafkaAdmin {
    config: AdminKafkaConfig,
    client: Client<DefaultClientContext>,
    topic_config: Vec<AdminTopicConfig>,
}

impl KafkaAdmin {
    pub async fn create_topics(&self) -> anyhow::Result<Vec<String>> {
        let mut client_config = ClientConfig::new();
        client_config.set("bootstrap_servers", self.config.bootstrap_servers.join(","));

        let mut topic_config: Vec<AdminTopicConfig> = Vec::new();
        topic_config.push(AdminTopicConfig {
            name: String::from("connect"),
            partitions: 1,
            replication_factor: 1,
        });

        let admin_client = AdminClient::from_config(&client_config)?;
        let topic_list = vec!["connect", "accept", "close", "bind", "listen"];
        let mut admin_opts = AdminOptions::new();

        let topics_list: Vec<_> = topic_config
            .iter()
            .map(|x| {
                NewTopic::new(
                    &x.name,
                    x.partitions,
                    TopicReplication::Fixed(x.replication_factor),
                )
            })
            .collect();

        if let Ok(_) = admin_client.create_topics(&topics_list, &admin_opts).await {
            return Ok(topic_list.iter().map(|x| x.to_string()).collect());
        } else {
            return Err(anyhow::bail!("failed to create topics"));
        }
    }

    pub async fn delete_topics(&self, topics_list: Vec<&str>) -> anyhow::Result<bool> {
        let mut client_config = ClientConfig::new();
        client_config.set("bootstrap_servers", self.config.bootstrap_servers.join(","));

        let admin_client = AdminClient::from_config(&client_config)?;

        if let Ok(_) = admin_client
            .delete_topics(&topics_list, &AdminOptions::new())
            .await
        {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    pub async fn list_topics(&self) -> anyhow::Result<Vec<String>> {
        let metadata = self.client.fetch_metadata(None, Duration::from_secs(5))?;
        Ok(metadata
            .topics()
            .iter()
            .map(|topic| topic.name().to_string())
            .collect())
    }

    pub async fn create_consumer(&self) -> anyhow::Result<StreamConsumer> {
        if let Some(consumer_config) = &self.config.consumer_config {
            let mut client_config = ClientConfig::new();
            client_config.set(
                "bootstrap_servers",
                &self.config.bootstrap_servers.join(","),
            );

            let mut config = consumer_config.key_value.iter().map(|x| {
                client_config
                    .set(x.0.clone(), x.1.clone())
                    .create::<StreamConsumer>()
                    .with_context(|| "Failed to create consumer")
                    .unwrap()
            });

            return Ok(config.next().with_context(|| "Failed to create consumer")?);
        } else {
            return Err(anyhow::bail!("Failed to create consumer"));
        }
    }

    pub async fn create_producer(&self) -> anyhow::Result<FutureProducer> {
        if let Some(producer_config) = &self.config.producer_config {
            let mut client_config = ClientConfig::new();
            client_config.set(
                "bootstrap_servers",
                &self.config.bootstrap_servers.join(","),
            );

            let mut config = producer_config.key_value.iter().map(|x| {
                client_config
                    .set(x.0.clone(), x.1.clone())
                    .create::<FutureProducer>()
                    .with_context(|| "Failed to create producer")
                    .unwrap()
            });

            return Ok(config.next().with_context(|| "Failed to create producer")?);
        } else {
            return Err(anyhow::bail!("Failed to create producer"));
        }
    }

    pub fn consumer_group_status(&self) -> anyhow::Result<()> {
        todo!()
    }
    pub fn consumer_lag(&self) -> anyhow::Result<()> {
        todo!()
    }
}
