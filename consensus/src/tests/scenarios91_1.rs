//RUST_BACKTRACE=1 cargo test test_block_validity -- --nocapture

use super::{mock_protocol_controller::MockProtocolController, tools};
use crate::start_consensus_controller;
use crypto::hash::Hash;
use models::Slot;
use time::UTime;

//use time::UTime;

#[tokio::test]
async fn test_ti() {
    /*    stderrlog::new()
    .verbosity(4)
    .timestamp(stderrlog::Timestamp::Millisecond)
    .init()
    .unwrap(); */

    let (mut cfg, serialization_context) = tools::default_consensus_config(1);
    cfg.t0 = 32000.into();
    cfg.delta_f0 = 32;
    //to avoir timing pb for block in the future
    cfg.genesis_timestamp = UTime::now()
        .unwrap()
        .saturating_sub(cfg.t0.checked_mul(1000).unwrap());

    // mock protocol
    let (mut protocol_controller, protocol_command_sender, protocol_event_receiver) =
        MockProtocolController::new(serialization_context.clone());

    // launch consensus controller
    let (consensus_command_sender, consensus_event_receiver, consensus_manager) =
        start_consensus_controller(
            cfg.clone(),
            serialization_context.clone(),
            protocol_command_sender.clone(),
            protocol_event_receiver,
            None,
        )
        .await
        .expect("could not start consensus controller");

    let genesis_hashes = consensus_command_sender
        .get_block_graph_status()
        .await
        .expect("could not get block graph status")
        .genesis_blocks;

    //create a valids block for thread 0
    let valid_hasht0s1 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 0),
        genesis_hashes.clone(),
        true,
        false,
    )
    .await;

    //create a valid block on the other thread.
    let valid_hasht1s1 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 1),
        genesis_hashes.clone(),
        true,
        false,
    )
    .await;

    //one click with 2 block compatible
    let block_graph = consensus_command_sender
        .get_block_graph_status()
        .await
        .unwrap();
    let block1_clic = tools::get_cliques(&block_graph, valid_hasht0s1);
    let block2_clic = tools::get_cliques(&block_graph, valid_hasht1s1);
    assert_eq!(1, block1_clic.len());
    assert_eq!(1, block2_clic.len());
    assert_eq!(block1_clic, block2_clic);

    //Create other clique bock T0S2
    let (fork_block_hash, block, _) = tools::create_block_with_merkle_root(
        &cfg,
        &serialization_context,
        Hash::hash("Other hash!".as_bytes()),
        Slot::new(2, 0),
        genesis_hashes.clone(),
    );

    protocol_controller.receive_block(block).await;
    tools::validate_propagate_block(&mut protocol_controller, fork_block_hash, 1000).await;
    //two clique with valid_hasht0s1 and valid_hasht1s1 in one and fork_block_hash, valid_hasht1s1 in the other
    //test the first clique hasn't changed.
    let block_graph = consensus_command_sender
        .get_block_graph_status()
        .await
        .unwrap();
    let block1_clic = tools::get_cliques(&block_graph, valid_hasht0s1);
    let block2_clic = tools::get_cliques(&block_graph, valid_hasht1s1);
    assert_eq!(1, block1_clic.len());
    assert_eq!(2, block2_clic.len());
    assert!(block2_clic.intersection(&block1_clic).next().is_some());
    //test the new click
    let fork_clic = tools::get_cliques(&block_graph, fork_block_hash);
    assert_eq!(1, fork_clic.len());
    assert!(fork_clic.intersection(&block1_clic).next().is_none());
    assert!(fork_clic.intersection(&block2_clic).next().is_some());

    //extend first clique
    let mut parentt0sn_hash = valid_hasht0s1;
    for period in 3..=35 {
        let block_hash = tools::create_and_test_block(
            &mut protocol_controller,
            &cfg,
            &serialization_context,
            Slot::new(period, 0),
            vec![parentt0sn_hash, valid_hasht1s1],
            true,
            false,
        )
        .await;
        //validate the added block isn't in the forked block click.
        let block_graph = consensus_command_sender
            .get_block_graph_status()
            .await
            .unwrap();
        let block_clic = tools::get_cliques(&block_graph, block_hash);
        let fork_clic = tools::get_cliques(&block_graph, fork_block_hash);
        assert!(fork_clic.intersection(&block_clic).next().is_none());

        parentt0sn_hash = block_hash;
    }

    //create new block in other clique
    let (invalid_block_hasht1s2, block, _) = tools::create_block(
        &cfg,
        &serialization_context,
        Slot::new(2, 1),
        vec![fork_block_hash, valid_hasht1s1],
    );
    protocol_controller.receive_block(block).await;
    assert!(
        !tools::validate_notpropagate_block(
            &mut protocol_controller,
            invalid_block_hasht1s2,
            1000,
        )
        .await
    );
    //verify that the clique has been pruned.
    let block_graph = consensus_command_sender
        .get_block_graph_status()
        .await
        .unwrap();
    let fork_clic = tools::get_cliques(&block_graph, fork_block_hash);
    assert_eq!(0, fork_clic.len());

    // stop controller while ignoring all commands
    let stop_fut = consensus_manager.stop(consensus_event_receiver);
    tokio::pin!(stop_fut);
    protocol_controller
        .ignore_commands_while(stop_fut)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_gpi() {
    // // setup logging
    /*stderrlog::new()
    .verbosity(4)
    .timestamp(stderrlog::Timestamp::Millisecond)
    .init()
    .unwrap();*/

    let (mut cfg, serialization_context) = tools::default_consensus_config(1);
    cfg.t0 = 32000.into();
    cfg.delta_f0 = 32;

    // to avoid timing problems for blocks in the future
    cfg.genesis_timestamp = UTime::now()
        .unwrap()
        .saturating_sub(cfg.t0.checked_mul(1000).unwrap());

    // mock protocol
    let (mut protocol_controller, protocol_command_sender, protocol_event_receiver) =
        MockProtocolController::new(serialization_context.clone());

    // launch consensus controller
    let (consensus_command_sender, consensus_event_receiver, consensus_manager) =
        start_consensus_controller(
            cfg.clone(),
            serialization_context.clone(),
            protocol_command_sender.clone(),
            protocol_event_receiver,
            None,
        )
        .await
        .expect("could not start consensus controller");

    let genesis_hashes = consensus_command_sender
        .get_block_graph_status()
        .await
        .expect("could not get block graph status")
        .genesis_blocks;

    // * create 1 normal block in each thread (t0s1 and t1s1) with genesis parents
    //create a valids block for thread 0
    let valid_hasht0s1 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 0),
        genesis_hashes.clone(),
        true,
        false,
    )
    .await;

    //create a valid block on the other thread.
    let valid_hasht1s1 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 1),
        genesis_hashes.clone(),
        true,
        false,
    )
    .await;

    //one click with 2 block compatible
    let block_graph = consensus_command_sender
        .get_block_graph_status()
        .await
        .unwrap();
    let block1_clic = tools::get_cliques(&block_graph, valid_hasht0s1);
    let block2_clic = tools::get_cliques(&block_graph, valid_hasht1s1);
    assert_eq!(1, block1_clic.len());
    assert_eq!(1, block2_clic.len());
    assert_eq!(block1_clic, block2_clic);

    //create 2 clique
    // * create 1 block in t0s2 with parents of slots (t0s1, t1s0)
    let valid_hasht0s2 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(2, 0),
        vec![valid_hasht0s1, genesis_hashes[1]],
        true,
        false,
    )
    .await;
    // * create 1 block in t1s2 with parents of slots (t0s0, t1s1)
    let valid_hasht1s2 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(2, 1),
        vec![genesis_hashes[0], valid_hasht1s1],
        true,
        false,
    )
    .await;

    // * after processing the block in t1s2, the block of t0s2 is incompatible with block of t1s2 (link in gi)
    let block_graph = consensus_command_sender
        .get_block_graph_status()
        .await
        .unwrap();
    let blockt1s2_clic = tools::get_cliques(&block_graph, valid_hasht1s2);
    let blockt0s2_clic = tools::get_cliques(&block_graph, valid_hasht0s2);
    assert!(blockt1s2_clic
        .intersection(&blockt0s2_clic)
        .next()
        .is_none());
    // * after processing the block in t1s2, there are 2 cliques, one with block of t0s2 and one with block of t1s2, and the parent vector uses the clique of minimum hash sum so the block of minimum hash between t0s2 and t1s2
    assert_eq!(1, blockt1s2_clic.len());
    assert_eq!(1, blockt0s2_clic.len());
    let parents = block_graph.best_parents.clone();
    if valid_hasht1s2 > valid_hasht0s2 {
        assert_eq!(parents[0], valid_hasht0s2)
    } else {
        assert_eq!(parents[1], valid_hasht1s2)
    }

    // * continue with 33 additional blocks in thread 0, that extend the clique of the block in t0s2:
    //    - a block in slot t0sX has parents (t0sX-1, t1s1), for X from 3 to 35
    let mut parentt0sn_hash = valid_hasht0s2;
    for period in 3..=35 {
        let block_hash = tools::create_and_test_block(
            &mut protocol_controller,
            &cfg,
            &serialization_context,
            Slot::new(period, 0),
            vec![parentt0sn_hash, valid_hasht1s1],
            true,
            false,
        )
        .await;
        parentt0sn_hash = block_hash;
    }
    // * create 1 block in t1s2 with the genesis blocks as parents
    tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(3, 1),
        vec![valid_hasht0s1, valid_hasht1s2],
        false,
        false,
    )
    .await;

    // * after processing the 33 blocks, one clique is removed (too late),
    //   the block of minimum hash becomes final, the one of maximum hash becomes stale
    //verify that the clique has been pruned.
    let block_graph = consensus_command_sender
        .get_block_graph_status()
        .await
        .unwrap();
    let fork_clic = tools::get_cliques(&block_graph, valid_hasht1s2);
    assert_eq!(0, fork_clic.len());
    assert!(block_graph
        .discarded_blocks
        .map
        .contains_key(&valid_hasht1s2));
    assert!(block_graph.active_blocks.contains_key(&valid_hasht0s2));
    assert!(!block_graph.active_blocks.contains_key(&valid_hasht1s2));

    // stop controller while ignoring all commands
    let stop_fut = consensus_manager.stop(consensus_event_receiver);
    tokio::pin!(stop_fut);
    protocol_controller
        .ignore_commands_while(stop_fut)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_old_stale() {
    // // setup logging
    // stderrlog::new()
    //     .verbosity(4)
    //     .timestamp(stderrlog::Timestamp::Millisecond)
    //     .init()
    //     .unwrap();

    let (mut cfg, serialization_context) = tools::default_consensus_config(1);
    cfg.t0 = 32000.into();
    cfg.delta_f0 = 32;

    //to avoid timing problems for blocks in the future
    cfg.genesis_timestamp = UTime::now()
        .unwrap()
        .saturating_sub(cfg.t0.checked_mul(1000).unwrap());

    // mock protocol
    let (mut protocol_controller, protocol_command_sender, protocol_event_receiver) =
        MockProtocolController::new(serialization_context.clone());

    // launch consensus controller
    let (consensus_command_sender, consensus_event_receiver, consensus_manager) =
        start_consensus_controller(
            cfg.clone(),
            serialization_context.clone(),
            protocol_command_sender.clone(),
            protocol_event_receiver,
            None,
        )
        .await
        .expect("could not start consensus controller");

    let genesis_hashes = consensus_command_sender
        .get_block_graph_status()
        .await
        .expect("could not get block graph status")
        .genesis_blocks;

    // * create 40 normal blocks in each thread: in slot 1 they have genesis parents, in slot 2 they have slot 1 parents
    //create a valid block for slot 1
    let mut valid_hasht0 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 0),
        genesis_hashes.clone(),
        true,
        false,
    )
    .await;

    //create a valid block on the other thread.
    let mut valid_hasht1 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 1),
        genesis_hashes.clone(),
        true,
        false,
    )
    .await;

    // and loop for the 39 other blocks
    for i in 0..39 {
        valid_hasht0 = tools::create_and_test_block(
            &mut protocol_controller,
            &cfg,
            &serialization_context,
            Slot::new(i + 2, 0),
            vec![valid_hasht0, valid_hasht1],
            true,
            false,
        )
        .await;

        //create a valid block on the other thread.
        valid_hasht1 = tools::create_and_test_block(
            &mut protocol_controller,
            &cfg,
            &serialization_context,
            Slot::new(i + 2, 1),
            vec![valid_hasht0, valid_hasht1],
            true,
            false,
        )
        .await;
    }

    //create 1 block in thread 0 slot 1 with genesis parents
    let _valid_hasht0s2 = tools::create_and_test_block(
        &mut protocol_controller,
        &cfg,
        &serialization_context,
        Slot::new(1, 0),
        genesis_hashes.clone(),
        false,
        false,
    )
    .await;

    // stop controller while ignoring all commands
    let stop_fut = consensus_manager.stop(consensus_event_receiver);
    tokio::pin!(stop_fut);
    protocol_controller
        .ignore_commands_while(stop_fut)
        .await
        .unwrap();
}