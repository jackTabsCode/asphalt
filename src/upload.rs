use std::{path::PathBuf, time::Duration};

use rbxcloud::rbx::assets::{
    create_asset, get_asset, AssetCreation, AssetCreationContext, AssetCreator, AssetType,
    AssetUserCreator, CreateAssetParams, GetAssetParams,
};

pub async fn upload_asset(path: PathBuf, asset_type: AssetType, api_key: String) -> String {
    let path_str = path.to_str().unwrap();

    let create_params = CreateAssetParams {
        api_key: api_key.clone(),
        filepath: path_str.to_string(),
        asset: AssetCreation {
            asset_type,
            display_name: path_str.to_string(),
            creation_context: AssetCreationContext {
                creator: AssetCreator::User(AssetUserCreator {
                    user_id: "9670971".to_string(),
                }),
                expected_price: None,
            },
            description: "Hey".to_string(),
        },
    };
    let operation = create_asset(&create_params).await.unwrap();
    let id = operation
        .path
        .unwrap()
        .split_once('/')
        .unwrap()
        .1
        .to_string();

    let create_params = GetAssetParams {
        api_key,
        operation_id: id,
    };

    loop {
        if let Ok(asset_operation) = get_asset(&create_params).await {
            if let Some(done) = asset_operation.done {
                if done {
                    return asset_operation.response.unwrap().asset_id;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
