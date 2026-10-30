//! O dicionário de texto compartilhado.
//!
//! Tradução de `src/core/pool.ts`. Cada célula de texto guarda um id
//! numérico em vez do texto em si; o `StringPool` traduz um lado no outro,
//! pra não repetir a mesma string mil vezes na memória.
//!
//! No TypeScript existe um `globalStringPool` só — uma variável de módulo,
//! compartilhada por todas as abas da pasta de trabalho. Rust não tem estado
//! global mutável sem `unsafe` ou um `Mutex`/`OnceLock`, e nenhum dos dois
//! serve aqui. Em vez disso, quem guarda célula (o [`crate::planilha::data_table::DataTable`]
//! de hoje, a `Sheet`/`Workbook` de amanhã) recebe o pool por referência. É
//! diferença de posse, não de comportamento: o pool continua sendo um só,
//! compartilhado — só que passado explicitamente, em vez de importado.

use std::{collections::HashMap, sync::Arc};

/// O maior id que cabe no `i32` da coluna de texto. Era 100 mil, copiado do
/// `pool.ts`, e passado dele a célula de texto não era guardada — sumia calada
/// (CSV do Spotify, 23/09/2026: coluna de ID única passa disso em 40 mil
/// linhas). O dicionário guarda o que a planilha tem; teto nele é perda de
/// dado, não economia.
const MAX_POOL_SIZE: usize = i32::MAX as usize;

/// Em JavaScript, `text.length` mede unidades UTF-16, não bytes UTF-8.
/// O limite de internacao do motor antigo era 256 dessas unidades.
const MAX_INTERN_UTF16: usize = 256;

/// Consulta no máximo 257 unidades mesmo para um texto de megabytes.
/// Textos curtos em bytes usam o caminho rápido e dispensam a varredura.
fn deve_internar(texto: &str) -> bool {
    texto.len() <= MAX_INTERN_UTF16
        || texto.encode_utf16().take(MAX_INTERN_UTF16 + 1).count() <= MAX_INTERN_UTF16
}

#[derive(Default)]
pub struct StringPool {
    // O índice reverso e a tabela compartilham a MESMA alocação por texto.
    // Antes havia dois Strings/duas cópias para cada valor único.
    str_to_id: HashMap<Arc<str>, i32>,
    id_to_str: Vec<Arc<str>>,
}

impl StringPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lugar para `n` textos novos de uma vez. Sem isso, o mapa dobra no meio de uma gravação
    /// longa e recalcula o hash de todos os textos num quadro só (72 ms com 800 mil textos,
    /// no Texto para Colunas de 83 colunas, #134).
    pub fn reservar(&mut self, n: usize) {
        self.str_to_id.reserve(n);
        self.id_to_str.reserve(n);
    }

    /// Devolve o id do texto, criando uma entrada nova se preciso. `-1` só
    /// passados 2 bilhões de textos diferentes (o `i32` da coluna de texto).
    pub fn get_or_create(&mut self, text: &str) -> i32 {
        if let Some(&id) = self.str_to_id.get(text) {
            return id;
        }
        if self.id_to_str.len() >= MAX_POOL_SIZE {
            return -1;
        }
        let id = self.id_to_str.len() as i32;
        let compartilhado: Arc<str> = Arc::from(text);
        self.id_to_str.push(Arc::clone(&compartilhado));
        self.str_to_id.insert(compartilhado, id);
        id
    }

    /// `""` para `id < 0` ou fora do pool — nunca erro, como no TypeScript
    /// (`?? ''`).
    pub fn get(&self, id: i32) -> &str {
        if id < 0 {
            return "";
        }
        self.id_to_str.get(id as usize).map(Arc::as_ref).unwrap_or("")
    }

    /// Retorna o dicionário de textos como Vec<String> para ser gravado no `.fshz`.
    pub fn to_vec(&self) -> Vec<String> {
        self.id_to_str.iter().map(|s| s.to_string()).collect()
    }

    /// Evita repetir a mesma string curta várias vezes; textos longos (mais
    /// de 256 unidades UTF-16, como `String.length` do TS) passam direto,
    /// sem entrar no pool. Isso inclui acentos e caracteres suplementares.
    ///
    /// Diferença de posse, não de comportamento: no TypeScript `intern`
    /// devolve o mesmo objeto de string já guardado (economiza alocação); em
    /// Rust `String` não tem esse compartilhamento sem indireção extra
    /// (`Rc<str>`), então aqui devolve uma cópia do texto — o valor é
    /// idêntico, só não é o mesmo objeto na memória.
    pub fn intern(&mut self, text: &str) -> String {
        if !deve_internar(text) {
            return text.to_string();
        }
        let id = self.get_or_create(text);
        if id < 0 {
            return text.to_string();
        }
        self.id_to_str[id as usize].to_string()
    }

    pub fn size(&self) -> usize {
        self.id_to_str.len()
    }

    pub fn clear(&mut self) {
        self.str_to_id.clear();
        self.id_to_str.clear();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cria_e_reaproveita_id() {
        let mut pool = StringPool::new();
        let a = pool.get_or_create("olá");
        let b = pool.get_or_create("olá");
        assert_eq!(a, b);
        assert_eq!(pool.get(a), "olá");
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn dicionario_compartilha_mesma_string_entre_os_indices() {
        let mut pool = StringPool::new();
        for i in 0..10_000 {
            pool.get_or_create(&format!("Produto-{i}-Descrição"));
        }
        assert_eq!(pool.size(), 10_000);
        let original = &pool.id_to_str[0];
        let chave = pool.str_to_id.get_key_value(original.as_ref()).unwrap().0;
        assert!(Arc::ptr_eq(original, chave),
            "HashMap e Vec devem compartilhar a mesma alocação");
        assert_eq!(pool.get(9_999), "Produto-9999-Descrição");
        assert_eq!(pool.to_vec().len(), 10_000);
    }

    #[test]
    fn id_negativo_e_fora_do_pool_devolvem_vazio() {
        let pool = StringPool::new();
        assert_eq!(pool.get(-1), "");
        assert_eq!(pool.get(999), "");
    }

    #[test]
    fn intern_texto_longo_nao_entra_no_pool() {
        let mut pool = StringPool::new();
        let longo = "x".repeat(300);
        let devolvido = pool.intern(&longo);
        assert_eq!(devolvido, longo);
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn internacao_tem_paridade_com_o_tamanho_utf16_do_typescript() {
        let mut pool = StringPool::new();

        // 200 letras acentuadas: 400 bytes UTF-8, mas 200 unidades UTF-16.
        let acentuado = "é".repeat(200);
        assert_eq!(pool.intern(&acentuado), acentuado);
        assert_eq!(pool.size(), 1);
        assert_eq!(pool.intern(&acentuado), acentuado);
        assert_eq!(pool.size(), 1, "texto repetido reutiliza o id");

        // O limite de 256 é inclusivo; acima dele o texto não entra no pool.
        let no_limite = "á".repeat(256);
        assert_eq!(pool.intern(&no_limite), no_limite);
        assert_eq!(pool.size(), 2);
        let acima = "á".repeat(257);
        assert_eq!(pool.intern(&acima), acima);
        assert_eq!(pool.size(), 2);

        // Emoji fora do BMP usa duas unidades UTF-16 por caractere.
        let emoji_no_limite = "😀".repeat(128);
        assert_eq!(pool.intern(&emoji_no_limite), emoji_no_limite);
        assert_eq!(pool.size(), 3);
        let emoji_acima = "😀".repeat(129);
        assert_eq!(pool.intern(&emoji_acima), emoji_acima);
        assert_eq!(pool.size(), 3);
    }

    #[test]
    fn apos_limpar_ids_sao_reutilizados_sem_texto_antigo() {
        let mut pool = StringPool::new();
        assert_eq!(pool.get_or_create("primeiro"), 0);
        pool.clear();
        assert_eq!(pool.get(0), "");
        assert_eq!(pool.get_or_create("segundo"), 0);
        assert_eq!(pool.get(0), "segundo");
    }

    #[test]
    fn clear_esvazia() {
        let mut pool = StringPool::new();
        pool.get_or_create("a");
        pool.clear();
        assert_eq!(pool.size(), 0);
    }
}
