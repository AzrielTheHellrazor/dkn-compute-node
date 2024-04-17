use clients::waku::WakuClient;

mod clients;
mod node;
mod utils;

#[tokio::main]
async fn main() {
    let waku = WakuClient::new(None);
    // call waku.health
    let health = waku.health();
    let result = health.await.unwrap();
    assert!(result.0, "Node is not healthy.");

    // relayed
    // let msgs = waku
    //     .relay
    //     .get_messages("/dria/1/synthesis/protobuf")
    //     .await
    //     .unwrap();
    // println!("Messages: {:?}", msgs);

    // stored
    let msgs = waku
        .store
        .get_messages("/dria/1/synthesis/protobuf", Some(true), None)
        .await
        .unwrap();
    println!("Messages: {:?}", msgs);
}
