#![allow(unused)]
use crate::parser::EvenType;
use rdkafka::{
    ClientConfig,
    producer::{FutureProducer, FutureRecord},
};
use std::time::Duration;

pub async fn connect_kafka(
    data: Vec<u8>,
    topic_name: &str,
    key: &str,
    producer: &FutureProducer,
) -> anyhow::Result<()> {
    producer
        .send(
            FutureRecord::to(topic_name).payload(&data).key(key),
            Duration::from_secs(0),
        )
        .await
        .unwrap();

    Ok(())
}
