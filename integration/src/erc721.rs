use crate::infrastructure::*;
use ethers::prelude::*;
use eyre::{bail, Result};

#[tokio::test]
async fn receive_context() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = 123.into();

    let name = infra.first.name().await?;
    let symbol = infra.first.symbol().await?;
    let token_uri = infra.first.token_uri(token_id).await?;

    assert_eq!(name, "PausableBurnableNft");
    assert_eq!(symbol, "PBN");
    assert_eq!(
        token_uri,
        "wwww.pbn.io/".to_string() + &token_id.to_string()
    );
    Ok(())
}

#[tokio::test]
async fn mint_nft_and_check_balance() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = random_token_id();
    let _ = infra
        .first
        .mint(infra.first.wallet.address(), token_id)
        .await?;
    let owner = infra.first.owner_of(token_id).await?;
    assert_eq!(owner, infra.first.wallet.address());

    let balance = infra.first.balance_of(infra.first.wallet.address()).await?;
    assert!(balance >= U256::one());
    Ok(())
}

#[tokio::test]
async fn error_mint_second_nft() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = random_token_id();
    let _ = infra
        .first
        .mint(infra.first.wallet.address(), token_id)
        .await?;
    match infra
        .first
        .mint(infra.first.wallet.address(), token_id)
        .await
    {
        Ok(_) => {
            bail!("Second mint of the same token should not be possible")
        }
        Err(e) => e.assert_has(ERC721InvalidSender {
            sender: Address::zero(),
        }),
    }
}

#[tokio::test]
async fn transfer_nft() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = random_token_id();
    let _ = infra
        .first
        .mint(infra.first.wallet.address(), token_id)
        .await?;
    let _ = infra
        .first
        .transfer_from(
            infra.first.wallet.address(),
            infra.second.wallet.address(),
            token_id,
        )
        .await?;
    let owner = infra.second.owner_of(token_id).await?;
    assert_eq!(owner, infra.second.wallet.address());
    Ok(())
}

#[tokio::test]
async fn error_transfer_nonexistent_nft() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = random_token_id();
    match infra
        .first
        .transfer_from(
            infra.first.wallet.address(),
            infra.second.wallet.address(),
            token_id,
        )
        .await
    {
        Ok(_) => {
            bail!("Transfer of a non existent nft should not be possible")
        }
        Err(e) => e.assert_has(ERC721NonexistentToken { token_id }),
    }
}

#[tokio::test]
async fn approve_nft_transfer() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = random_token_id();
    let _ = infra
        .first
        .mint(infra.first.wallet.address(), token_id)
        .await?;
    let _ = infra
        .first
        .approve(infra.second.wallet.address(), token_id)
        .await?;
    let _ = infra
        .second
        .transfer_from(
            infra.first.wallet.address(),
            infra.second.wallet.address(),
            token_id,
        )
        .await?;
    let owner = infra.second.owner_of(token_id).await?;
    assert_eq!(owner, infra.second.wallet.address());
    Ok(())
}

#[tokio::test]
async fn error_not_approved_nft_transfer() -> Result<()> {
    let infra = Infrastructure::create().await?;
    let token_id = random_token_id();
    let _ = infra
        .first
        .mint(infra.first.wallet.address(), token_id)
        .await?;
    match infra
        .second
        .transfer_from(
            infra.first.wallet.address(),
            infra.second.wallet.address(),
            token_id,
        )
        .await
    {
        Ok(_) => {
            bail!("Transfer of not approved token should not happen")
        }
        Err(e) => e.assert_has(ERC721InsufficientApproval {
            operator: infra.second.wallet.address(),
            token_id,
        }),
    }
}