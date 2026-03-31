//! Discord Rich Presence manager for Ragnarok Online.
//!
//! Manages the Discord IPC connection and updates presence with game data.
//! Map name translations ported from HorizonRichPresence MapNames.h.
//! Custom maps and Discord app settings are loaded from kpatcher.yml.

#![cfg(windows)]

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::memory_reader::GameData;
use crate::patcher::DiscordConfiguration;

/// Get or create the singleton built-in map translations table.
fn builtin_map_translations() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::with_capacity(350);
        // --- SPECIAL & TUTORIAL ---
        m.insert("mainprt", "Cidade Inicial");
        m.insert("poring_c01", "Vila dos Porings");
        m.insert("poring_c02", "Vila dos Porings");
        // --- RUNE-MIDGARTS ---
        m.insert("prontera", "Prontera");
        m.insert("prt_in", "Interior de Prontera");
        m.insert("prt_castle", "Castelo de Prontera");
        m.insert("prt_church", "Igreja de Prontera");
        m.insert("prt_fild01", "Arredores de Prontera 01");
        m.insert("prt_fild02", "Arredores de Prontera 02");
        m.insert("prt_fild03", "Arredores de Prontera 03");
        m.insert("prt_fild04", "Arredores de Prontera 04");
        m.insert("prt_fild05", "Arredores de Prontera 05");
        m.insert("prt_fild06", "Arredores de Prontera 06");
        m.insert("prt_fild07", "Arredores de Prontera 07");
        m.insert("prt_fild08", "Arredores de Prontera 08");
        m.insert("prt_fild09", "Arredores de Prontera 09");
        m.insert("prt_fild10", "Arredores de Prontera 10");
        m.insert("prt_fild11", "Arredores de Prontera 11");
        m.insert("prt_maze01", "Labirinto da Floresta 1");
        m.insert("prt_maze02", "Labirinto da Floresta 2");
        m.insert("prt_maze03", "Labirinto da Floresta 3");
        m.insert("prt_sewb1", "Esgoto de Prontera 1");
        m.insert("prt_sewb2", "Esgoto de Prontera 2");
        m.insert("prt_sewb3", "Esgoto de Prontera 3");
        m.insert("prt_sewb4", "Esgoto de Prontera 4");
        m.insert("izlude", "Izlude");
        m.insert("iz_dun00", "Túnel Submarino 1");
        m.insert("iz_dun01", "Túnel Submarino 2");
        m.insert("iz_dun02", "Túnel Submarino 3");
        m.insert("iz_dun03", "Túnel Submarino 4");
        m.insert("iz_dun04", "Túnel Submarino 5");
        m.insert("geffen", "Geffen");
        m.insert("gef_tower", "Torre de Geffen");
        m.insert("gef_fild00", "Arredores de Geffen 00");
        m.insert("gef_fild01", "Arredores de Geffen 01");
        m.insert("gef_fild02", "Arredores de Geffen 02");
        m.insert("gef_fild03", "Arredores de Geffen 03");
        m.insert("gef_fild04", "Arredores de Geffen 04");
        m.insert("gef_fild05", "Arredores de Geffen 05");
        m.insert("gef_fild06", "Arredores de Geffen 06");
        m.insert("gef_fild07", "Arredores de Geffen 07");
        m.insert("gef_fild08", "Arredores de Geffen 08");
        m.insert("gef_fild09", "Arredores de Geffen 09");
        m.insert("gef_fild10", "Vila dos Orcs");
        m.insert("gef_fild11", "Arredores de Geffen 11");
        m.insert("gef_fild12", "Arredores de Geffen 12");
        m.insert("gef_fild13", "Arredores de Geffen 13");
        m.insert("gef_fild14", "Vila dos Orcs - Oeste");
        m.insert("gef_dun00", "Calabouço de Geffen 1");
        m.insert("gef_dun01", "Calabouço de Geffen 2");
        m.insert("gef_dun02", "Calabouço de Geffen 3");
        m.insert("orcsdun01", "Caverna dos Orcs 1");
        m.insert("orcsdun02", "Caverna dos Orcs 2");
        m.insert("gefenia01", "Gefenia 1");
        m.insert("gefenia02", "Gefenia 2");
        m.insert("gefenia03", "Gefenia 3");
        m.insert("gefenia04", "Gefenia 4");
        m.insert("payon", "Payon");
        m.insert("pay_arche", "Vila dos Arqueiros");
        m.insert("pay_fild01", "Floresta de Payon 01");
        m.insert("pay_fild02", "Floresta de Payon 02");
        m.insert("pay_fild03", "Floresta de Payon 03");
        m.insert("pay_fild04", "Floresta de Payon 04");
        m.insert("pay_fild05", "Floresta de Payon 05");
        m.insert("pay_fild06", "Floresta de Payon 06");
        m.insert("pay_fild07", "Floresta de Payon 07");
        m.insert("pay_fild08", "Floresta de Payon 08");
        m.insert("pay_fild09", "Floresta de Payon 09");
        m.insert("pay_fild10", "Floresta de Payon 10");
        m.insert("pay_fild11", "Floresta de Payon 11");
        m.insert("pay_dun00", "Caverna de Payon 1");
        m.insert("pay_dun01", "Caverna de Payon 2");
        m.insert("pay_dun02", "Caverna de Payon 3");
        m.insert("pay_dun03", "Caverna de Payon 4");
        m.insert("pay_dun04", "Caverna de Payon 5");
        m.insert("morocc", "Morroc");
        m.insert("moc_ruins", "Ruínas de Morroc");
        m.insert("moc_fild01", "Deserto de Sograt 01");
        m.insert("moc_fild02", "Deserto de Sograt 02");
        m.insert("moc_fild03", "Deserto de Sograt 03");
        m.insert("moc_fild04", "Deserto de Sograt 04");
        m.insert("moc_fild05", "Deserto de Sograt 05");
        m.insert("moc_fild06", "Deserto de Sograt 06");
        m.insert("moc_fild07", "Deserto de Sograt 07");
        m.insert("moc_fild08", "Deserto de Sograt 08");
        m.insert("moc_fild09", "Deserto de Sograt 09");
        m.insert("moc_fild10", "Deserto de Sograt 10");
        m.insert("moc_fild11", "Deserto de Sograt 11");
        m.insert("moc_fild12", "Deserto de Sograt 12");
        m.insert("moc_fild13", "Deserto de Sograt 13");
        m.insert("moc_fild14", "Deserto de Sograt 14");
        m.insert("moc_fild15", "Deserto de Sograt 15");
        m.insert("moc_fild16", "Deserto de Sograt 16");
        m.insert("moc_fild17", "Deserto de Sograt 17");
        m.insert("moc_fild18", "Deserto de Sograt 18");
        m.insert("moc_fild19", "Deserto de Sograt 19");
        m.insert("moc_pryd01", "Pirâmide 1");
        m.insert("moc_pryd02", "Pirâmide 2");
        m.insert("moc_pryd03", "Pirâmide 3");
        m.insert("moc_pryd04", "Pirâmide 4");
        m.insert("moc_pryd05", "Pirâmide 5");
        m.insert("moc_prydb1", "Subsolo da Pirâmide 1");
        m.insert("moc_prydb2", "Subsolo da Pirâmide 2");
        m.insert("moc_prydb_fild01", "Templo das Valquírias");
        m.insert("in_sphinx1", "Esfinge 1");
        m.insert("in_sphinx2", "Esfinge 2");
        m.insert("in_sphinx3", "Esfinge 3");
        m.insert("in_sphinx4", "Esfinge 4");
        m.insert("in_sphinx5", "Esfinge 5");
        m.insert("anthell01", "Formigueiro Infernal 1");
        m.insert("anthell02", "Formigueiro Infernal 2");
        m.insert("alberta", "Alberta");
        m.insert("alb2trea", "Ilha de Alberta");
        m.insert("treasure01", "Navio Fantasma 1");
        m.insert("treasure02", "Navio Fantasma 2");
        m.insert("tur_dun01", "Ilha da Tartaruga 1");
        m.insert("tur_dun02", "Ilha da Tartaruga 2");
        m.insert("tur_dun03", "Ilha da Tartaruga 3");
        m.insert("tur_dun04", "Ilha da Tartaruga 4");
        m.insert("comodo", "Comodo");
        m.insert("cmd_fild01", "Pântano de Papuchica");
        m.insert("cmd_fild02", "Fortaleza de Saint Darmain (Leste)");
        m.insert("cmd_fild03", "Fortaleza de Saint Darmain (Oeste)");
        m.insert("cmd_fild04", "Caverna de Kokomo");
        m.insert("cmd_fild05", "Floresta de Papuchica");
        m.insert("cmd_fild06", "Fortaleza de Saint Darmain (Sul)");
        m.insert("cmd_fild07", "Farol de Pharos");
        m.insert("cmd_fild08", "Zenhai");
        m.insert("cmd_fild09", "Fortaleza de Saint Darmain (Norte)");
        m.insert("beach_dun", "Caverna do Norte");
        m.insert("beach_dun2", "Caverna do Oeste");
        m.insert("beach_dun3", "Caverna do Leste");
        m.insert("umbala", "Umbala");
        m.insert("um_fild01", "Arredores de Umbala 01");
        m.insert("um_fild02", "Arredores de Umbala 02");
        m.insert("um_fild03", "Arredores de Umbala 03");
        m.insert("um_fild04", "Arredores de Umbala 04");
        m.insert("um_dun01", "Carpinteiro");
        m.insert("um_dun02", "Passagem para Yggdrasil");
        m.insert("yggdrasil01", "Yggdrasil");
        m.insert("niflheim", "Niflheim");
        m.insert("nif_fild01", "Arredores de Niflheim 01");
        m.insert("nif_fild02", "Arredores de Niflheim 02");
        m.insert("glast_01", "Glast Heim");
        m.insert("gl_cas01", "Glast Heim 1F");
        m.insert("gl_cas02", "Glast Heim 2F");
        m.insert("gl_church", "Igreja de Glast Heim");
        m.insert("gl_chyard", "Cemitério de Glast Heim");
        m.insert("gl_knt01", "Cavalaria de Glast Heim 1");
        m.insert("gl_knt02", "Cavalaria de Glast Heim 2");
        m.insert("gl_dun01", "Caverna de Glast Heim 1");
        m.insert("gl_dun02", "Caverna de Glast Heim 2");
        m.insert("gl_prison", "Prisão Subterrânea 1");
        m.insert("gl_prison1", "Prisão Subterrânea 2");
        m.insert("gl_sew01", "Esgoto de Glast Heim 1");
        m.insert("gl_sew02", "Esgoto de Glast Heim 2");
        m.insert("gl_sew03", "Esgoto de Glast Heim 3");
        m.insert("gl_sew04", "Esgoto de Glast Heim 4");
        m.insert("gl_step", "Escadaria de Glast Heim");
        // --- SCHWARZWALD ---
        m.insert("aldebaran", "Al De Baran");
        m.insert("c_tower1", "Torre do Relógio 1");
        m.insert("c_tower2", "Torre do Relógio 2");
        m.insert("c_tower3", "Torre do Relógio 3");
        m.insert("c_tower4", "Torre do Relógio 4");
        m.insert("alde_dun01", "Subsolo da Torre do Relógio 1");
        m.insert("alde_dun02", "Subsolo da Torre do Relógio 2");
        m.insert("alde_dun03", "Subsolo da Torre do Relógio 3");
        m.insert("alde_dun04", "Subsolo da Torre do Relógio 4");
        m.insert("yuno", "Juno");
        m.insert("yuno_fild01", "Arredores de Juno 01");
        m.insert("yuno_fild02", "Arredores de Juno 02");
        m.insert("yuno_fild03", "Arredores de Juno 03");
        m.insert("yuno_fild04", "Arredores de Juno 04");
        m.insert("mag_dun01", "Caverna de Magma 1");
        m.insert("mag_dun02", "Caverna de Magma 2");
        m.insert("juperos_01", "Ruínas de Juperos 1");
        m.insert("juperos_02", "Ruínas de Juperos 2");
        m.insert("jupe_core", "Núcleo de Juperos");
        m.insert("einbroch", "Einbroch");
        m.insert("einbech", "Einbech");
        m.insert("ein_dun01", "Mina de Einbech 1");
        m.insert("ein_dun02", "Mina de Einbech 2");
        m.insert("lighthalzen", "Lighthalzen");
        m.insert("lhz_fild01", "Arredores de Lighthalzen 01");
        m.insert("lhz_fild02", "Arredores de Lighthalzen 02");
        m.insert("lhz_fild03", "Arredores de Lighthalzen 03");
        m.insert("lhz_dun01", "Biolaboratório 1");
        m.insert("lhz_dun02", "Biolaboratório 2");
        m.insert("lhz_dun03", "Biolaboratório 3");
        m.insert("lhz_dun04", "Biolaboratório 4");
        m.insert("hugel", "Hugel");
        m.insert("hu_fild01", "Arredores de Hugel 01");
        m.insert("hu_fild02", "Arredores de Hugel 02");
        m.insert("hu_fild03", "Arredores de Hugel 03");
        m.insert("hu_fild04", "Arredores de Hugel 04");
        m.insert("hu_fild05", "Arredores de Hugel 05");
        m.insert("hu_fild06", "Arredores de Hugel 06");
        m.insert("odyn_tem01", "Templo de Odin 1");
        m.insert("odyn_tem02", "Templo de Odin 2");
        m.insert("odyn_tem03", "Templo de Odin 3");
        m.insert("abyss_01", "Lago do Abismo 1");
        m.insert("abyss_02", "Lago do Abismo 2");
        m.insert("abyss_03", "Lago do Abismo 3");
        m.insert("tha_t01", "Torre de Thanatos 1");
        m.insert("tha_t02", "Torre de Thanatos 2");
        m.insert("tha_t03", "Torre de Thanatos 3");
        m.insert("tha_t04", "Torre de Thanatos 4");
        m.insert("tha_t05", "Torre de Thanatos 5");
        m.insert("tha_t06", "Torre de Thanatos 6");
        m.insert("tha_t07", "Torre de Thanatos 7");
        m.insert("tha_t08", "Torre de Thanatos 8");
        m.insert("tha_t09", "Torre de Thanatos 9");
        m.insert("tha_t10", "Torre de Thanatos 10");
        m.insert("tha_t11", "Torre de Thanatos 11");
        m.insert("tha_t12", "Torre de Thanatos 12");
        m.insert("kh_dun01", "Fábrica de Robôs 1");
        m.insert("kh_dun02", "Fábrica de Robôs 2");
        // --- ARUNAFELTZ ---
        m.insert("rachel", "Rachel");
        m.insert("ra_fild01", "Planície de Ida 01");
        m.insert("ra_fild02", "Planície de Ida 02");
        m.insert("ra_fild03", "Planície de Ida 03");
        m.insert("ra_fild04", "Planície de Ida 04");
        m.insert("ra_fild05", "Planície de Ida 05");
        m.insert("ra_fild06", "Planície de Ida 06");
        m.insert("ra_fild07", "Planície de Ida 07");
        m.insert("ra_fild08", "Planície de Ida 08");
        m.insert("ra_fild09", "Planície de Ida 09");
        m.insert("ra_fild10", "Planície de Ida 10");
        m.insert("ra_fild11", "Planície de Ida 11");
        m.insert("ra_fild12", "Planície de Ida 12");
        m.insert("ra_san01", "Santuário de Rachel 1");
        m.insert("ra_san02", "Santuário de Rachel 2");
        m.insert("ra_san03", "Santuário de Rachel 3");
        m.insert("ra_san04", "Santuário de Rachel 4");
        m.insert("ra_san05", "Santuário de Rachel 5");
        m.insert("ice_dun01", "Caverna de Gelo 1");
        m.insert("ice_dun02", "Caverna de Gelo 2");
        m.insert("ice_dun03", "Caverna de Gelo 3");
        m.insert("veins", "Veins");
        m.insert("ve_fild01", "Campos de Veins 01");
        m.insert("ve_fild02", "Campos de Veins 02");
        m.insert("ve_fild03", "Campos de Veins 03");
        m.insert("ve_fild04", "Campos de Veins 04");
        m.insert("ve_fild05", "Campos de Veins 05");
        m.insert("ve_fild06", "Campos de Veins 06");
        m.insert("ve_fild07", "Campos de Veins 07");
        m.insert("thor_v01", "Vulcão de Thor 1");
        m.insert("thor_v02", "Vulcão de Thor 2");
        m.insert("thor_v03", "Vulcão de Thor 3");
        // --- GLOBAL PROJECT ---
        m.insert("brasilis", "Brasilis");
        m.insert("bra_fild01", "Campo de Brasilis");
        m.insert("bra_dun01", "Floresta Amazônica");
        m.insert("bra_dun02", "Subsolo de Brasilis");
        m.insert("amatsu", "Amatsu");
        m.insert("ama_dun01", "Tatami");
        m.insert("ama_dun02", "Campo de Batalha Subterrâneo");
        m.insert("ama_dun03", "Santuário Subterrâneo");
        m.insert("gonryun", "Kunlun");
        m.insert("gon_fild01", "Campo de Kunlun");
        m.insert("gon_dun01", "Santuário de Kunlun 1");
        m.insert("gon_dun02", "Santuário de Kunlun 2");
        m.insert("gon_dun03", "Santuário de Kunlun 3");
        m.insert("louyang", "Louyang");
        m.insert("lou_fild01", "Campo de Louyang");
        m.insert("lou_dun01", "Tumba Real");
        m.insert("lou_dun02", "Interior da Tumba Real");
        m.insert("lou_dun03", "Coração da Tumba Real");
        m.insert("ayothaya", "Ayothaya");
        m.insert("ayo_fild01", "Campo de Ayothaya 1");
        m.insert("ayo_fild02", "Campo de Ayothaya 2");
        m.insert("ayo_dun01", "Antigo Santuário 1");
        m.insert("ayo_dun02", "Antigo Santuário 2");
        m.insert("moscovia", "Moscóvia");
        m.insert("mosk_fild01", "Campo de Moscóvia");
        m.insert("mosk_dun01", "Floresta Encantada 1");
        m.insert("mosk_dun02", "Floresta Encantada 2");
        m.insert("mosk_dun03", "Floresta Encantada 3");
        m.insert("dewata", "Dewata");
        m.insert("dew_fild01", "Campo de Dewata");
        m.insert("dew_dun01", "Caverna de Krakatau 1");
        m.insert("dew_dun02", "Caverna de Krakatau 2");
        m.insert("malaya", "Porto Malaya");
        m.insert("ma_fild01", "Campo de Malaya 1");
        m.insert("ma_fild02", "Campo de Malaya 2");
        m.insert("ma_dun01", "Hospital de Bangungot");
        // --- NEW WORLD ---
        m.insert("mid_camp", "Acampamento da Expedição");
        m.insert("manuk", "Manuk");
        m.insert("man_fild01", "Arredores de Manuk 01");
        m.insert("man_fild02", "Arredores de Manuk 02");
        m.insert("man_fild03", "Arredores de Manuk 03");
        m.insert("splendide", "Esplendor");
        m.insert("spl_fild01", "Arredores de Esplendor 01");
        m.insert("spl_fild02", "Arredores de Esplendor 02");
        m.insert("spl_fild03", "Arredores de Esplendor 03");
        m.insert("dicastes01", "El Dicastes");
        m.insert("dic_fild01", "Arredores de El Dicastes 01");
        m.insert("dic_fild02", "Arredores de El Dicastes 02");
        m.insert("dic_dun01", "Calabouço de Kamidal 1");
        m.insert("dic_dun02", "Calabouço de Kamidal 2");
        m.insert("mora", "Mora");
        m.insert("eclage", "Eclage");
        m.insert("ecl_fild01", "Arredores de Eclage 01");
        // --- INSTANCES ---
        m.insert("moc_para01", "Grupo do Éden");
        m.insert("1@tower", "Torre sem Fim (1-25)");
        m.insert("2@tower", "Torre sem Fim (26-50)");
        m.insert("3@tower", "Torre sem Fim (51-75)");
        m.insert("4@tower", "Torre sem Fim (76-100)");
        m.insert("5@tower", "Torre sem Fim (Topo)");
        m.insert("1@gl_k", "Antiga Glast Heim (Jardim)");
        m.insert("2@gl_k", "Antiga Glast Heim (Castelo)");
        m.insert("1@nyd", "Ninho de Nidhogg 1");
        m.insert("2@nyd", "Ninho de Nidhogg 2");
        m.insert("1@orcs", "Memória dos Orcs 1");
        m.insert("2@orcs", "Memória dos Orcs 2");
        m.insert("1@md_pryd", "Pirâmide (Pesadelo) 1");
        m.insert("2@md_pryd", "Pirâmide (Pesadelo) 2");
        m
    })
}

/// Translate a raw map filename to a human-readable name.
///
/// Lookup order: custom_maps (from config) → built-in table → capitalize fallback.
pub fn get_map_name(raw_map: &str, custom_maps: &HashMap<String, String>) -> String {
    if raw_map.is_empty() {
        return String::new();
    }

    // Strip .rsw / .gat extension if present
    let cleaned = raw_map
        .trim_end_matches(".rsw")
        .trim_end_matches(".gat")
        .trim_end_matches(".RSW")
        .trim_end_matches(".GAT");

    let lower = cleaned.to_lowercase();

    // 1. Check custom maps first (user-defined overrides / custom maps)
    if let Some(translated) = custom_maps.get(&lower) {
        return translated.clone();
    }

    // 2. Check built-in translations
    if let Some(&translated) = builtin_map_translations().get(lower.as_str()) {
        return translated.to_string();
    }

    // 3. Fallback: capitalize first letter
    let mut chars = cleaned.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut s = first.to_uppercase().to_string();
            s.extend(chars);
            s
        }
    }
}

/// Manages the Discord IPC connection and presence updates.
pub struct DiscordRpcManager {
    client: DiscordIpcClient,
    connected: bool,
    start_timestamp: i64,
    config: DiscordConfiguration,
}

impl DiscordRpcManager {
    pub fn new(config: DiscordConfiguration) -> Self {
        let client = DiscordIpcClient::new(&config.client_id);
        let start_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            client,
            connected: false,
            start_timestamp,
            config,
        }
    }

    /// Attempt to connect to Discord IPC.
    pub fn connect(&mut self) -> bool {
        if self.connected {
            return true;
        }
        match self.client.connect() {
            Ok(_) => {
                self.connected = true;
                // Reset timestamp on new connection
                self.start_timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                log::info!("[RichPresence] Connected to Discord IPC");
                true
            }
            Err(e) => {
                log::warn!("[RichPresence] Failed to connect to Discord: {}", e);
                false
            }
        }
    }

    /// Update the Discord Rich Presence with the current game data.
    pub fn update_presence(&mut self, game_data: &GameData) {
        if !self.connected {
            return;
        }

        let (details, state) = if game_data.is_in_login {
            (
                "Escolhendo Char".to_string(),
                "Selecionando Personagem".to_string(),
            )
        } else {
            let map_display = get_map_name(&game_data.map_name, &self.config.custom_maps);
            (
                format!(
                    "Char: {} | Lv {}/{}",
                    game_data.player_name, game_data.base_level, game_data.job_level
                ),
                format!("Mapa: {}", map_display),
            )
        };

        let activity = Activity::new()
            .details(&details)
            .state(&state)
            .assets(
                Assets::new()
                    .large_image(&self.config.large_image)
                    .large_text(&self.config.large_text)
                    .small_image(&self.config.small_image)
                    .small_text(&self.config.small_text),
            )
            .timestamps(Timestamps::new().start(self.start_timestamp));

        if let Err(e) = self.client.set_activity(activity) {
            log::warn!("[RichPresence] Failed to update presence: {}", e);
            // Connection may be broken
            self.connected = false;
        }
    }

    /// Clear the Rich Presence (shows nothing on Discord).
    pub fn clear_presence(&mut self) {
        if !self.connected {
            return;
        }
        let _ = self.client.clear_activity();
    }

    /// Disconnect from Discord IPC.
    pub fn disconnect(&mut self) {
        if self.connected {
            self.clear_presence();
            let _ = self.client.close();
            self.connected = false;
            log::info!("[RichPresence] Disconnected from Discord IPC");
        }
    }
}

impl Drop for DiscordRpcManager {
    fn drop(&mut self) {
        self.disconnect();
    }
}
