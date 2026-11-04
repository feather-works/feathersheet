//! Duplicar uma aba: a cópia independente de uma [`Sheet`].
//!
//! `Sheet` não é `Clone` (colunas em blocos, grafo de dependências e cache
//! de cálculo não se clonam de graça). A cópia é refeita pelo caminho de
//! sempre: cada célula ocupada volta por [`Sheet::snapshot`] (texto digitado
//! + estilo direto) e entra por [`Sheet::apply`], que reconstrói coluna,
//! pool de texto, grafo e limites. O que não é célula (larguras, alturas,
//! mesclas, formatação condicional, tabelas, validação, botões, imagens,
//! estilos de linha/coluna/aba, cor da aba) é copiado campo a campo.
//! A cópia nasce sem histórico de desfazer.

use super::*;

impl Sheet {
    /// Uma cópia completa desta aba com o nome `nome`. Fórmulas são copiadas
    /// como estão: as que citam outra aba (ou esta pelo nome) continuam
    /// apontando para onde apontavam.
    pub fn duplicar(&self, nome: impl Into<String>) -> Sheet {
        self.duplicar_renomeando(nome, &[])
    }

    /// Os nomes das tabelas desta aba (campo `tb`).
    pub fn nomes_das_tabelas(&self) -> Vec<String> {
        self.tb.as_deref().map(nomes_de_tabelas_em_tb).unwrap_or_default()
    }

    /// Como [`Self::duplicar`], trocando os nomes de tabela `(antigo, novo)`
    /// na cópia: no campo `tb` e nas referências estruturadas (`Antigo[Col]`)
    /// das fórmulas da própria cópia. Referências com aba (`Outra!T[c]`) não
    /// mudam.
    pub fn duplicar_renomeando(&self, nome: impl Into<String>, tabelas: &[(String, String)]) -> Sheet {
        let mut copia = Sheet::new(nome);
        copia.tab_color = self.tab_color.clone();
        copia.col_widths = self.col_widths.clone();
        copia.largura_padrao = self.largura_padrao;
        copia.topicos = self.topicos.clone();
        copia.protecao = self.protecao.clone();
        copia.comentarios = self.comentarios.clone();
        copia.row_heights = self.row_heights.clone();
        copia.linhas_ocultas_desde = self.linhas_ocultas_desde;
        copia.colunas_ocultas_desde = self.colunas_ocultas_desde;
        copia.col_styles = self.col_styles.clone();
        copia.row_styles = self.row_styles.clone();
        copia.sheet_style = self.sheet_style.clone();
        copia.g = self.g.clone();
        copia.pr = self.pr.clone();
        copia.r = self.r.clone();
        copia.tb = self.tb.as_deref().map(|tb| renomear_no_tb(tb, tabelas));
        copia.oq = self.oq.clone();
        copia.dn = self.dn.clone();
        copia.mc = self.mc.clone();
        copia.im = self.im.clone();
        copia.va = self.va.clone();
        copia.bt = self.bt.clone();
        copia.tn = self.tn.clone();
        copia.ob = self.ob.clone();
        copia.mg = self.mg.clone();
        copia.ce = self.ce.clone();

        let mut celulas: Vec<(u32, u32, edicao::Snap)> = Vec::new();
        self.for_each_snapshot(&mut |row, col, snap| celulas.push((row, col, snap.clone())));
        for (row, col, mut snap) in celulas {
            if !tabelas.is_empty() && snap.raw.starts_with('=') {
                snap.raw = renomear_em_formula(&snap.raw, tabelas);
            }
            copia.apply(row, col, Some(snap), false);
        }
        // O que `apply` marcou como tocado não é mudança do usuário.
        *copia.rastro.borrow_mut() = Rastro::default();
        copia
    }
}

/// Campos de texto das tabelas do `tb` (lista de objetos), sem JSON: para
/// cada objeto de nível 2, `(chave, início, fim)` do conteúdo (sem aspas) de
/// valores string de chave `nome`/`displayName`/`id`. Objetos aninhados
/// (`area`, `estilo`, `filtros`) são pulados por contagem de profundidade.
fn campos_de_texto(tb: &str) -> Vec<(usize, String, usize, usize)> {
    let b = tb.as_bytes();
    let (mut i, mut prof, mut obj, mut out) = (0, 0usize, 0usize, Vec::new());
    let ler = |mut k: usize| -> (usize, usize) {
        // b[k] == '"': devolve (início, fim) do conteúdo; fim é a aspa final.
        let ini = k + 1;
        k += 1;
        while k < b.len() && b[k] != b'"' {
            k += if b[k] == b'\\' { 2 } else { 1 };
        }
        (ini, k.min(b.len()))
    };
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => {
                prof += 1;
                if b[i] == b'{' && prof == 2 { obj += 1; }
                i += 1;
            }
            b'}' | b']' => { prof = prof.saturating_sub(1); i += 1; }
            b'"' => {
                let (ini, fim) = ler(i);
                let mut k = fim + 1;
                while k < b.len() && b[k].is_ascii_whitespace() { k += 1; }
                if prof == 2 && k < b.len() && b[k] == b':' {
                    let chave = &tb[ini..fim];
                    if matches!(chave, "nome" | "displayName" | "id") {
                        k += 1;
                        while k < b.len() && b[k].is_ascii_whitespace() { k += 1; }
                        if k < b.len() && b[k] == b'"' {
                            let (vi, vf) = ler(k);
                            out.push((obj, chave.to_string(), vi, vf));
                            i = vf + 1;
                            continue;
                        }
                    }
                }
                i = fim + 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// Os `nome` das tabelas do `tb`.
fn nomes_de_tabelas_em_tb(tb: &str) -> Vec<String> {
    campos_de_texto(tb).into_iter()
        .filter(|(_, c, i, f)| c == "nome" && f > i)
        .map(|(_, _, i, f)| tb[i..f].to_string())
        .collect()
}

/// Troca `nome`/`displayName` (e o `id` no padrão `tab_<nome>`) no `tb`.
fn renomear_no_tb(tb: &str, tabelas: &[(String, String)]) -> String {
    if tabelas.is_empty() {
        return tb.to_string();
    }
    let campos = campos_de_texto(tb);
    let novo_de = |obj: usize| -> Option<(&str, &str)> {
        let (_, _, i, f) = campos.iter().find(|(o, c, _, _)| *o == obj && c == "nome")?;
        let antigo = &tb[*i..*f];
        tabelas.iter().find(|(a, _)| a.eq_ignore_ascii_case(antigo))
            .map(|(a, n)| (a.as_str(), n.as_str()))
    };
    let mut trocas: Vec<(usize, usize, String)> = Vec::new();
    for (obj, chave, i, f) in &campos {
        let Some((_, novo)) = novo_de(*obj) else { continue };
        let atual = &tb[*i..*f];
        match chave.as_str() {
            "nome" | "displayName" => trocas.push((*i, *f, novo.to_string())),
            _ if atual.starts_with("tab_") => trocas.push((*i, *f, format!("tab_{novo}"))),
            _ => {}
        }
    }
    let mut out = String::with_capacity(tb.len());
    let mut pos = 0;
    for (i, f, texto) in trocas {
        out.push_str(&tb[pos..i]);
        out.push_str(&texto);
        pos = f;
    }
    out.push_str(&tb[pos..]);
    out
}

/// Reescreve `Antigo[...]` para `Novo[...]` numa fórmula, ignorando texto
/// entre aspas, nomes colados a outro identificador e referências com aba.
fn renomear_em_formula(f: &str, tabelas: &[(String, String)]) -> String {
    let cs: Vec<char> = f.chars().collect();
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    let mut out = String::with_capacity(f.len());
    let (mut i, mut aspas) = (0, false);
    while i < cs.len() {
        let c = cs[i];
        if c == '"' {
            aspas = !aspas;
        }
        if !aspas && ident(c) && (i == 0 || !(ident(cs[i - 1]) || cs[i - 1] == '!' || cs[i - 1] == '\'')) {
            let mut j = i;
            while j < cs.len() && ident(cs[j]) {
                j += 1;
            }
            let palavra: String = cs[i..j].iter().collect();
            if cs.get(j) == Some(&'[') {
                if let Some((_, novo)) = tabelas.iter().find(|(a, _)| a.eq_ignore_ascii_case(&palavra)) {
                    out.push_str(novo);
                    i = j;
                    continue;
                }
            }
            out.push_str(&palavra);
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}
