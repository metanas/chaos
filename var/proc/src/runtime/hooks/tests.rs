use super::*;

#[tokio::test]
async fn hooks_cas_approval_and_delete_recreate() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let db = RuntimeDbHandle::Sqlite(StateRuntime::init(home.path().into(), "test".into()).await?);
    let definition: HookDefinition = serde_json::from_value(serde_json::json!({
        "event": "before_turn", "command": "printf hello"
    }))?;
    let first = db
        .commit_hook("hello", 0, Some(&definition), true, "one")
        .await?;
    assert!(db.list_hooks("one").await?[0].approved);
    assert!(!db.list_hooks("two").await?[0].approved);
    assert!(
        db.commit_hook("hello", 0, Some(&definition), true, "one")
            .await
            .is_err()
    );
    db.revoke_approvals("one", None).await?;
    assert!(!db.list_hooks("one").await?[0].approved);
    let second = db
        .commit_hook("hello", first, Some(&definition), true, "two")
        .await?;
    assert!(!db.list_hooks("one").await?[0].approved);
    assert!(db.list_hooks("two").await?[0].approved);
    assert!(
        db.commit_hook("hello", first, None, false, "one")
            .await
            .is_err()
    );
    db.commit_hook("hello", second, None, false, "two").await?;
    let third = db
        .commit_hook("hello", 0, Some(&definition), false, "one")
        .await?;
    assert!(third > second);
    assert!(!db.list_hooks("two").await?[0].approved);
    assert!(
        db.import_hooks(&[
            ("fresh".into(), definition.clone()),
            ("hello".into(), definition)
        ])
        .await
        .is_err()
    );
    assert_eq!(
        db.list_hooks("one").await?.len(),
        1,
        "import must roll back as a unit"
    );
    Ok(())
}
