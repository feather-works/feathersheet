//! Migração exata da criptografia de `ui/protecao/protecao.ts`.
//!
//! Apenas a derivação/validação de credenciais. Bloqueio de edição,
//! autorização por intervalo e diálogos pertencem a outra etapa:
//! não anunciar proteção ativa antes de conectar esse controle ao motor.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use unicode_normalization::UnicodeNormalization;

/// Contrato do TypeScript, preservado no JSON `Sheet.pr` de arquivos FSHZ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SenhaGuardada {
    pub alg: String,
    pub iteracoes: u32,
    pub sal: String,
    pub hash: String,
}

pub const ITERACOES_PADRAO: u32 = 600_000;
const SAL_BYTES: usize = 16;
const HASH_BYTES: usize = 32;
// Arquivo malicioso não pode forçar centenas de milhões de iterações na UI.
const MAX_ITERACOES: u32 = 2_000_000;

fn validar_parametros(algoritmo: &str, iteracoes: u32) -> Result<(), String> {
    if algoritmo != "PBKDF2-SHA256" {
        return Err(tr!("Algoritmo de proteção não suportado.").into());
    }
    if !(1..=MAX_ITERACOES).contains(&iteracoes) {
        return Err(tr!("Número de iterações de proteção inválido.").into());
    }
    Ok(())
}

/// A senha **não** é aparada: espaços são parte dela. O TS normaliza NFC,
/// inclusive caracteres compostos e acentos combinantes.
fn derivar(senha: &str, sal: &[u8], iteracoes: u32) -> [u8; HASH_BYTES] {
    let normalizada = senha.nfc().collect::<String>();
    let mut out = [0_u8; HASH_BYTES];
    pbkdf2_hmac::<Sha256>(normalizada.as_bytes(), sal, iteracoes, &mut out);
    out
}

/// Útil para validar contra vetores dos testes do browser sem RNG.
#[cfg(test)]
fn criar_com_sal(senha: &str, sal: [u8; SAL_BYTES], iteracoes: u32)
    -> Result<SenhaGuardada, String>
{
    validar_parametros("PBKDF2-SHA256", iteracoes)?;
    if senha.is_empty() {
        return Err(tr!("Informe uma senha para proteger o intervalo.").into());
    }
    let hash = derivar(senha, &sal, iteracoes);
    Ok(SenhaGuardada {
        alg: "PBKDF2-SHA256".into(),
        iteracoes,
        sal: STANDARD.encode(sal),
        hash: STANDARD.encode(hash),
    })
}

/// Cria o mesmo objeto `SenhaGuardada` do TypeScript. O único segredo de
/// longo prazo é a senha do usuário, nunca persistida; o sal é de CSPRNG.
pub fn criar_senha(senha: &str) -> Result<SenhaGuardada, String> {
    if senha.is_empty() {
        return Err(tr!("Informe uma senha para proteger o intervalo.").into());
    }
    let mut sal = [0_u8; SAL_BYTES];
    getrandom::getrandom(&mut sal)
        .map_err(|_| tr!("O gerador criptográfico de números aleatórios não está disponível."))?;
    let hash = derivar(senha, &sal, ITERACOES_PADRAO);
    Ok(SenhaGuardada {
        alg: "PBKDF2-SHA256".into(),
        iteracoes: ITERACOES_PADRAO,
        sal: STANDARD.encode(sal),
        hash: STANDARD.encode(hash),
    })
}

/// Aceita hashes gravados no FSHZ original pela WebCrypto do TypeScript.
/// Só compara 32 bytes depois de validar algoritmo, tamanho e iterações.
pub fn conferir_senha(senha: &str, guardada: &SenhaGuardada) -> Result<bool, String> {
    validar_parametros(&guardada.alg, guardada.iteracoes)?;
    let sal = STANDARD.decode(&guardada.sal)
        .map_err(|_| tr!("Sal de proteção não é Base64 válido."))?;
    let hash = STANDARD.decode(&guardada.hash)
        .map_err(|_| tr!("Hash de proteção não é Base64 válido."))?;
    if sal.len() != SAL_BYTES || hash.len() != HASH_BYTES {
        return Err(tr!("Tamanho de sal ou hash da proteção é inválido.").into());
    }
    let obtido = derivar(senha, &sal, guardada.iteracoes);
    let mut diferenca: u8 = 0;
    for i in 0..HASH_BYTES {
        diferenca |= obtido[i] ^ hash[i];
    }
    Ok(diferenca == 0)
}

/// Retângulo serializado em `Sheet.pr` pelo TypeScript (base zero).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreaProtegida {
    pub r0: u32,
    pub c0: u32,
    pub r1: u32,
    pub c1: u32,
}

impl AreaProtegida {
    pub fn valida(&self) -> bool {
        use feathersheet_core::celulas::refs::{MAX_COLS, MAX_ROWS};
        self.r0 <= self.r1 && self.c0 <= self.c1
            && self.r1 < MAX_ROWS && self.c1 < MAX_COLS
    }

    pub fn cruza(&self, outra: &Self) -> bool {
        self.valida() && outra.valida()
            && self.r0 <= outra.r1 && self.r1 >= outra.r0
            && self.c0 <= outra.c1 && self.c1 >= outra.c0
    }

    pub fn contem(&self, linha: u32, coluna: u32) -> bool {
        self.valida()
            && (self.r0..=self.r1).contains(&linha)
            && (self.c0..=self.c1).contains(&coluna)
    }
}

/// Metadados legados, sem senha em texto puro, preservados nos arquivos FSHZ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtecaoDeIntervalo {
    pub id: String,
    pub area: AreaProtegida,
    #[serde(default)]
    pub descricao: String,
    pub senha: SenhaGuardada,
}

/// Importa o JSON do TypeScript sem desbloquear nenhum intervalo.
pub fn importar_intervalos(json: Option<&str>) -> Result<Vec<ProtecaoDeIntervalo>, String> {
    let Some(json) = json else { return Ok(Vec::new()); };
    if json.len() > 1024 * 1024 {
        return Err(tr!("Os metadados de proteção excedem 1 MiB.").into());
    }
    let protecoes: Vec<ProtecaoDeIntervalo> = serde_json::from_str(json)
        .map_err(|_| tr!("O JSON de proteções da aba está inválido."))?;
    if protecoes.len() > 4096 {
        return Err(tr!("Limite de 4096 intervalos protegidos ultrapassado.").into());
    }
    for (i, item) in protecoes.iter().enumerate() {
        if item.id.is_empty() || !item.area.valida() {
            return Err(tr!("Uma proteção contém ID ou coordenadas inválidas.").into());
        }
        validar_parametros(&item.senha.alg, item.senha.iteracoes)?;
        if protecoes[..i].iter().any(|previa| previa.id == item.id) {
            return Err(tr!("IDs duplicados nas proteções do arquivo.").into());
        }
    }
    Ok(protecoes)
}

/// Serializa apenas metadados, preservando o formato de array do TS.
pub fn exportar_intervalos(protecoes: &[ProtecaoDeIntervalo]) -> Result<Option<String>, String> {
    if protecoes.is_empty() { return Ok(None); }
    let json = serde_json::to_string(protecoes)
        .map_err(|_| tr!("Não foi possível serializar as proteções."))?;
    importar_intervalos(Some(&json))?;
    Ok(Some(json))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn vetor_pbkdf2_sha256_conhecido_interopera_com_browser() {
        // PBKDF2-HMAC-SHA256("password","salt",1,32)
        // O vetor clássico aplica sal de 4 bytes: verificar a função de
        // derivação diretamente, não a política de geração de sal de 16.
        let obtido = derivar("password", b"salt", 1);
        assert_eq!(obtido.as_slice(), &[
            0x12,0x0f,0xb6,0xcf,0xfc,0xf8,0xb3,0x2c,
            0x43,0xe7,0x22,0x52,0x56,0xc4,0xf8,0x37,
            0xa8,0x65,0x48,0xc9,0x2c,0xcc,0x35,0x48,
            0x08,0x05,0x98,0x7c,0xb7,0x0b,0xe1,0x7b
        ]);
    }

    #[test]
    fn json_legado_e_nfc_validam_sem_senha_em_claro() {
        let sal = [17_u8; 16];
        let guardada = criar_com_sal("cafe\u{301}  ", sal, 700).unwrap();
        let json = serde_json::to_string(&guardada).unwrap();
        assert!(!json.contains("café"));
        let importada: SenhaGuardada = serde_json::from_str(&json).unwrap();
        assert!(conferir_senha("café  ", &importada).unwrap());
        assert!(!conferir_senha("café ", &importada).unwrap());
        assert!(!conferir_senha("café  x", &importada).unwrap());
    }

    #[test]
    fn rng_e_sal_unicos_mesma_senha_600_mil_iteracoes() {
        let a = criar_senha("senha válida").unwrap();
        let b = criar_senha("senha válida").unwrap();
        assert_eq!(a.alg, "PBKDF2-SHA256");
        assert_eq!(a.iteracoes, ITERACOES_PADRAO);
        assert_ne!(a.sal, b.sal);
        assert_ne!(a.hash, b.hash);
        assert!(conferir_senha("senha válida", &a).unwrap());
        assert!(!conferir_senha("senha invalida", &a).unwrap());
    }

    #[test]
    fn vetor_600_mil_pode_ser_verificado_vindo_do_ts_sem_rehash() {
        // WebCrypto PBKDF2-HMAC-SHA256, NFC("cafe + acento + 2 espaços"),
        // sal de bytes 0..15 e 600_000 iterações, saída Base64 de 32 bytes.
        let guardada: SenhaGuardada = serde_json::from_str(
            r#"{"alg":"PBKDF2-SHA256","iteracoes":600000,
                "sal":"AAECAwQFBgcICQoLDA0ODw==",
                "hash":"yVMSfCFRypxePUd5BPTncA6p8hr4Sn7CugBPYQmg1p8="}"#
        ).unwrap();
        assert!(conferir_senha("café  ", &guardada).unwrap());
        assert!(conferir_senha("cafe\u{301}  ", &guardada).unwrap());
        assert!(!conferir_senha("café ", &guardada).unwrap());
    }

    #[test]
    fn metadados_fshz_do_ts_preservam_area_descricao_e_hash() {
        let senha = criar_com_sal("chave", [9; 16], 100).unwrap();
        let items = vec![ProtecaoDeIntervalo {
            id: "pr_abc123".into(),
            area: AreaProtegida { r0: 4, c0: 2, r1: 20, c1: 8 },
            descricao: "Resultados".into(),
            senha,
        }];
        let json = exportar_intervalos(&items).unwrap().unwrap();
        assert!(!json.contains("chave"));
        let restaurada = importar_intervalos(Some(&json)).unwrap();
        assert_eq!(restaurada, items);
        assert!(restaurada[0].area.contem(7, 6));
        assert!(!restaurada[0].area.contem(1, 6));
        assert!(restaurada[0].area.cruza(&AreaProtegida {
            r0: 19, c0: 8, r1: 22, c1: 9,
        }));
        let mut pasta = feathersheet_core::planilha::workbook::Workbook::new();
        pasta.aba_mut(0).unwrap().sheet.pr = Some(json);
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&pasta, 0).unwrap();
        let lida = feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap();
        let aberta = feathersheet_io::para_workbook::para_workbook(&lida);
        let restaurada = importar_intervalos(aberta.aba(0).unwrap().sheet.pr.as_deref()).unwrap();
        assert_eq!(restaurada, items);
        assert!(conferir_senha("chave", &restaurada[0].senha).unwrap());
    }

    #[test]
    fn nega_metadados_corrompidos_ou_custo_absurdo_antes_do_pbkdf2() {
        let mut item = criar_com_sal("senhazinha", [1;16], 1).unwrap();
        item.iteracoes = u32::MAX;
        assert!(conferir_senha("senhazinha", &item).is_err());
        item.iteracoes = 1;
        item.alg = "MD5".into();
        assert!(conferir_senha("senhazinha", &item).is_err());
        item.alg = "PBKDF2-SHA256".into();
        item.hash = "a".into();
        assert!(conferir_senha("senhazinha", &item).is_err());
        item.hash = STANDARD.encode([0u8; 1]);
        assert!(conferir_senha("senhazinha", &item).is_err());
    }
}
