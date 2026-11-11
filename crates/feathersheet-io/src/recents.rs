//! Gerenciamento da lista de arquivos recentes (G8 - sheet-io::recents).

use serde::{Deserialize, Serialize};

/// Informações sobre um arquivo recente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recente {
    /// Caminho completo do arquivo.
    pub caminho: String,
    /// Nome exibido (o do arquivo, sem extensão).
    pub nome: String,
    /// Quando foi aberto pela última vez, em milissegundos desde a época Unix.
    pub aberto: u64,
    /// Quantas abas tinha, para a lista dar uma pista do tamanho.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abas: Option<usize>,
}

/// Limite máximo de recentes mantidos na lista.
pub const MAX_RECENTES: usize = 12;

/// Põe um arquivo no topo. Se ele já estava na lista, sobe em vez de duplicar.
pub fn registrar(lista: &[Recente], entrada: Recente) -> Vec<Recente> {
    let mut out = Vec::with_capacity(lista.len() + 1);
    out.push(entrada.clone());
    for item in lista {
        if item.caminho != entrada.caminho {
            out.push(item.clone());
        }
    }
    out.truncate(MAX_RECENTES);
    out
}

/// Tira um arquivo da lista pelo caminho.
pub fn esquecer(lista: &[Recente], caminho: &str) -> Vec<Recente> {
    lista.iter().filter(|r| r.caminho != caminho).cloned().collect()
}

/// Descarta os arquivos que não existem mais no disco.
pub fn limpar<F>(lista: &[Recente], mut existe: F) -> Vec<Recente>
where
    F: FnMut(&str) -> bool,
{
    lista.iter().filter(|r| existe(&r.caminho)).cloned().collect()
}

/// Converte a lista para JSON.
pub fn para_json(lista: &[Recente]) -> String {
    serde_json::to_string(lista).unwrap_or_else(|_| "[]".to_string())
}

/// Lê a lista de um texto JSON guardado, ignorando lixo e garantindo limite.
pub fn da_json(texto: Option<&str>) -> Vec<Recente> {
    let Some(raw) = texto else { return Vec::new() };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };

    let Some(arr) = val.as_array() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for item in arr {
        let Some(obj) = item.as_object() else { continue };
        let caminho = match obj.get("caminho").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            _ => continue,
        };

        let nome = match obj.get("nome").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => caminho.clone(),
        };

        let aberto = obj.get("aberto").and_then(|v| v.as_u64()).unwrap_or(0);
        let abas = obj.get("abas").and_then(|v| v.as_u64()).map(|n| n as usize);

        out.push(Recente {
            caminho,
            nome,
            aberto,
            abas,
        });
    }

    out.truncate(MAX_RECENTES);
    out
}

/// Retorna descrição amigável de tempo decorrido ("agora", "há 5 minutos", "ontem", etc).
pub fn quando(aberto: u64, agora: u64) -> String {
    if agora <= aberto {
        return "agora".to_string();
    }

    let diff_ms = agora - aberto;
    let s = diff_ms / 1000;

    if s < 60 {
        return "agora".to_string();
    }

    let min = s / 60;
    if min < 60 {
        return if min == 1 {
            "há 1 minuto".to_string()
        } else {
            format!("há {} minutos", min)
        };
    }

    let h = min / 60;
    if h < 24 {
        return if h == 1 {
            "há 1 hora".to_string()
        } else {
            format!("há {} horas", h)
        };
    }

    let d = h / 24;
    if d == 1 {
        return "ontem".to_string();
    }

    if d < 7 {
        return format!("há {} dias", d);
    }

    // O TypeScript exibia a data local em pt-BR para entradas com 7 dias
    // ou mais. Sem acrescentar dependências de calendário ao motor IO,
    // calculamos a data civil UTC. O instante gravado permanece intacto.
    data_pt_br_utc(aberto).unwrap_or_else(|| feathersheet_core::trf!("há {d} dias"))
}

/// Converte os milissegundos Unix persistidos no histórico para DD/MM/AAAA.
/// Usa a rotina de calendário do core para evitar outra implementação
/// do calendário gregoriano. UTC evita depender do fuso da máquina e permite
/// que o mesmo IO rode num futuro destino WebAssembly.
fn data_pt_br_utc(aberto_ms: u64) -> Option<String> {
    // Fora do intervalo de datas usuais (até o ano 9999), não tentar
    // representar ano absurdo na lista de recentes.
    if aberto_ms > 253_402_300_799_999 {
        return None;
    }
    let serial_excel = (aberto_ms / 86_400_000) as i64 + 25_569;
    let (ano, mes, dia) = feathersheet_core::celulas::date_serial::serial_to_ymd(serial_excel);
    Some(format!("{dia:02}/{mes:02}/{ano:04}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registrar_e_limite() {
        let mut lista: Vec<Recente> = Vec::new();
        for i in 1..=15 {
            let item = Recente {
                caminho: format!("/path/file{}.xlsx", i),
                nome: format!("file{}", i),
                aberto: i * 100,
                abas: Some(1),
            };
            lista = registrar(&lista, item);
        }

        assert_eq!(lista.len(), MAX_RECENTES);
        assert_eq!(lista[0].caminho, "/path/file15.xlsx");
        assert_eq!(lista[11].caminho, "/path/file4.xlsx");
    }

    #[test]
    fn test_registrar_reordena_existente() {
        let item1 = Recente {
            caminho: "/path/a.xlsx".to_string(),
            nome: "a".to_string(),
            aberto: 100,
            abas: None,
        };
        let item2 = Recente {
            caminho: "/path/b.xlsx".to_string(),
            nome: "b".to_string(),
            aberto: 200,
            abas: None,
        };
        let lista = vec![item1.clone(), item2];

        let item1_atualizado = Recente {
            caminho: "/path/a.xlsx".to_string(),
            nome: "a novo".to_string(),
            aberto: 300,
            abas: Some(2),
        };

        let novalista = registrar(&lista, item1_atualizado.clone());
        assert_eq!(novalista.len(), 2);
        assert_eq!(novalista[0], item1_atualizado);
        assert_eq!(novalista[1].caminho, "/path/b.xlsx");
    }

    #[test]
    fn test_esquecer() {
        let item1 = Recente {
            caminho: "/a.xlsx".to_string(),
            nome: "a".to_string(),
            aberto: 100,
            abas: None,
        };
        let item2 = Recente {
            caminho: "/b.xlsx".to_string(),
            nome: "b".to_string(),
            aberto: 200,
            abas: None,
        };
        let lista = vec![item1, item2.clone()];
        let apos_esquecer = esquecer(&lista, "/a.xlsx");

        assert_eq!(apos_esquecer.len(), 1);
        assert_eq!(apos_esquecer[0], item2);
    }

    #[test]
    fn test_limpar() {
        let item1 = Recente {
            caminho: "/a.xlsx".to_string(),
            nome: "a".to_string(),
            aberto: 100,
            abas: None,
        };
        let item2 = Recente {
            caminho: "/b.xlsx".to_string(),
            nome: "b".to_string(),
            aberto: 200,
            abas: None,
        };
        let lista = vec![item1, item2.clone()];

        let limpa = limpar(&lista, |c| c == "/b.xlsx");
        assert_eq!(limpa.len(), 1);
        assert_eq!(limpa[0], item2);
    }

    #[test]
    fn test_json_serde() {
        let json_str = r#"[
            {"caminho": "/path/1.xlsx", "nome": "1", "aberto": 1000, "abas": 3},
            {"caminho": "/path/2.xlsx", "aberto": 2000}
        ]"#;

        let parsed = da_json(Some(json_str));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].caminho, "/path/1.xlsx");
        assert_eq!(parsed[0].abas, Some(3));
        assert_eq!(parsed[1].nome, "/path/2.xlsx");

        let reserialized = para_json(&parsed);
        assert!(reserialized.contains("/path/1.xlsx"));
    }

    #[test]
    fn data_de_recentes_antigos_em_portugues() {
        // 12/09/2026 00:00 UTC; oito dias depois deve mostrar a data.
        let aberto = 1_789_171_200_000_u64;
        let oito_dias = 8 * 86_400_000_u64;
        assert_eq!(quando(aberto, aberto + oito_dias), "12/09/2026");
        // Na fronteira exata de sete dias, deixa de ser "há 6 dias".
        assert_eq!(quando(aberto, aberto + 7 * 86_400_000), "12/09/2026");
        assert_eq!(quando(aberto, aberto + 6 * 86_400_000), "há 6 dias");
    }

    #[test]
    fn calendario_bissexto_e_timestamp_invalido() {
        // 29/02/2000 00:00 UTC
        assert_eq!(data_pt_br_utc(951_782_400_000), Some("29/02/2000".into()));
        assert_eq!(data_pt_br_utc(0), Some("01/01/1970".into()));
        assert_eq!(data_pt_br_utc(u64::MAX), None);
    }

    #[test]
    fn test_quando() {
        let agora = 100_000;
        assert_eq!(quando(100_000, agora), "agora");
        assert_eq!(quando(95_000, agora), "agora");
        assert_eq!(quando(40_000, agora), "há 1 minuto");
        assert_eq!(quando(0, 120_000), "há 2 minutos");
        assert_eq!(quando(0, 3_600_000), "há 1 hora");
        assert_eq!(quando(0, 86_400_000), "ontem");
        assert_eq!(quando(0, 86_400_000 * 3), "há 3 dias");
    }
}
