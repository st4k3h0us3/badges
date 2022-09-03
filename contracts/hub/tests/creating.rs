use cosmwasm_std::testing::{mock_dependencies, mock_info, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, Addr, DepsMut, Empty, OwnedDeps, Response, Timestamp};
use cw_utils::Expiration;
use sg_metadata::Metadata;

use badge_hub::contract;
use badge_hub::error::ContractError;
use badge_hub::state::*;
use badges::{Badge, MintRule};

mod utils;

fn setup_test() -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
    let mut deps = mock_dependencies();

    OWNER.save(deps.as_mut().storage, &Addr::unchecked("larry")).unwrap();
    NFT.save(deps.as_mut().storage, &Addr::unchecked("nft")).unwrap();
    BADGE_COUNT.save(deps.as_mut().storage, &0).unwrap();

    deps
}

fn mock_badge() -> Badge<Addr> {
    Badge {
        id: 1,
        manager: Addr::unchecked("larry"),
        metadata: Metadata {
            name: Some("first-badge".to_string()),
            ..Default::default()
        },
        rule: MintRule::ByKeys,
        expiry: Some(Expiration::AtTime(Timestamp::from_seconds(12345))),
        max_supply: Some(100),
        current_supply: 0,
    }
}

fn create_badge(deps: DepsMut, badge: &Badge<Addr>) -> Response {
    contract::create_badge(
        deps,
        mock_info("larry", &[]),
        badge.manager.to_string(),
        badge.metadata.clone(),
        badge.rule.clone(),
        badge.expiry,
        badge.max_supply,
    )
    .unwrap()
}

#[test]
fn creating_badge() {
    let mut deps = setup_test();

    // non-owner cannot create badge
    {
        let badge = mock_badge();

        let err = contract::create_badge(
            deps.as_mut(),
            mock_info("jake", &[]),
            badge.manager.to_string(),
            badge.metadata.clone(),
            badge.rule.clone(),
            badge.expiry,
            badge.max_supply,
        )
        .unwrap_err();
        assert_eq!(err, ContractError::NotOwner);
    }

    // create the first badge
    {
        let badge = Badge {
            id: 1,
            manager: Addr::unchecked("larry"),
            metadata: Metadata {
                name: Some("first-badge".to_string()),
                ..Default::default()
            },
            rule: MintRule::ByMinter("larry".to_string()),
            expiry: Some(Expiration::AtTime(Timestamp::from_seconds(12345))),
            max_supply: Some(100),
            current_supply: 0,
        };

        let res = create_badge(deps.as_mut(), &badge);
        assert_eq!(res.messages, vec![]);
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "badges/hub/create_badge"),
                attr("id", "1"),
                attr("manager", "larry"),
                attr("rule", "by_minter:larry"),
                attr("expiry", "expiration time: 12345.000000000"),
                attr("max_supply", "100")
            ]
        );

        let cfg = contract::query_config(deps.as_ref()).unwrap();
        assert_eq!(cfg.badge_count, 1);

        let b = contract::query_badge(deps.as_ref(), 1).unwrap();
        assert_eq!(b, badge.into());
    }

    // create the second badge
    {
        let badge = Badge {
            id: 2,
            manager: Addr::unchecked("jake"),
            metadata: Metadata {
                name: Some("second-badge".to_string()),
                ..Default::default()
            },
            rule: MintRule::ByKeys,
            expiry: None,
            max_supply: None,
            current_supply: 0,
        };

        let res = create_badge(deps.as_mut(), &badge);
        assert_eq!(res.messages, vec![]);
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "badges/hub/create_badge"),
                attr("id", "2"),
                attr("manager", "jake"),
                attr("rule", "by_keys"),
                attr("expiry", "undefined"),
                attr("max_supply", "undefined")
            ]
        );

        let cfg = contract::query_config(deps.as_ref()).unwrap();
        assert_eq!(cfg.badge_count, 2);

        let b = contract::query_badge(deps.as_ref(), 2).unwrap();
        assert_eq!(b, badge.into());
    }
}

#[test]
fn editing_badge() {
    let mut deps = setup_test();

    let badge = mock_badge();
    create_badge(deps.as_mut(), &badge);

    // non-manager cannot edit
    {
        let err = contract::edit_badge(
            deps.as_mut(),
            Addr::unchecked("jake"),
            badge.id,
            Metadata::default(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::NotManager);
    }

    // manager can edit
    {
        let res = contract::edit_badge(
            deps.as_mut(),
            badge.manager.clone(),
            badge.id,
            Metadata::default(),
        )
        .unwrap();
        assert_eq!(res.messages, vec![]);
        assert_eq!(res.attributes, vec![attr("action", "badges/hub/edit_badge"), attr("id", "1")]);

        let b = contract::query_badge(deps.as_ref(), 1).unwrap();
        assert_eq!(b.metadata, Metadata::default());
    }
}

#[test]
fn adding_keys() {
    let mut deps = setup_test();

    // badge 1 has mint rule "by keys"
    let mut badge = mock_badge();
    create_badge(deps.as_mut(), &badge);

    // badge 2 has mint rule "by minter"
    badge.rule = MintRule::ByMinter("pumpkin".to_string());
    create_badge(deps.as_mut(), &badge);

    // non-manager cannot add key
    {
        let err = contract::add_keys(
            deps.as_mut(),
            utils::mock_env_at_timestamp(10000),
            mock_info("jake", &[]),
            1,
            utils::hashset(&["1234abcd"]),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::NotManager);
    }

    // cannot add key if the badge is not of "by keys" mint rule
    {
        let err = contract::add_keys(
            deps.as_mut(),
            utils::mock_env_at_timestamp(10000),
            mock_info("larry", &[]),
            2,
            utils::hashset(&["1234abcd"]),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::wrong_mint_rule("by_keys", &badge.rule));
    }

    // cannot add key if the badge is no longer available
    {
        let err = contract::add_keys(
            deps.as_mut(),
            utils::mock_env_at_timestamp(99999),
            mock_info("larry", &[]),
            1,
            utils::hashset(&["1234abcd"]),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::Expired);
    }

    // cannot add invalid hex-encoded strings
    {
        let err = contract::add_keys(
            deps.as_mut(),
            utils::mock_env_at_timestamp(10000),
            mock_info("larry", &[]),
            1,
            utils::hashset(&["ngmi"]),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ContractError::FromHex(hex::FromHexError::InvalidHexCharacter {
                c: 'n',
                index: 0
            }),
        );
    }

    // manager properly adds keys
    {
        let res = contract::add_keys(
            deps.as_mut(),
            utils::mock_env_at_timestamp(10000),
            mock_info("larry", &[]),
            1,
            utils::hashset(&["1234abcd", "4321dcba"]),
        )
        .unwrap();
        assert_eq!(res.messages, vec![]);
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "badges/hub/add_keys"),
                attr("id", "1"),
                attr("keys_added", "2"),
            ],
        );

        let keys = contract::query_keys(deps.as_ref(), 1, None, None).unwrap();
        assert_eq!(keys, vec!["1234abcd".to_string(), "4321dcba".to_string()]);
    }
}