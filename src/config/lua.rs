use mlua::LuaSerdeExt;

use super::toml::ConfigFile;

pub(super) fn parse_lua_config(src: &str) -> Result<ConfigFile, String> {
    let lua = mlua::Lua::new();
    tracing::info!("config: lua eval start");
    let value = lua
        .load(src)
        .set_name("hypr-drift config")
        .eval::<mlua::Value>()
        .map_err(|e| {
            let msg = format!("lua eval error: {e}");
            tracing::error!("config: {msg}");
            msg
        })?;

    if matches!(value, mlua::Value::Nil) {
        tracing::warn!("config: lua returned nil; using defaults");
        return Ok(ConfigFile::default());
    }

    lua.from_value::<ConfigFile>(value).map_err(|e| {
        let msg = format!("lua config shape error: {e}");
        tracing::error!("config: {msg}");
        msg
    })
}
