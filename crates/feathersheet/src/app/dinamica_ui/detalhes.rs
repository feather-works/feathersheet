//! "Mostrar detalhes" da tabela dinâmica (16/10/2026, meta da v0.1.0): duplo
//! clique num valor abre uma aba nova com as linhas da origem que formam o
//! número, como no Excel. Os valores são copiados (não as fórmulas: as
//! referências apontariam para o lugar errado na aba nova).
use super::*;
use super::edicao::{ler_estado, localizar_na_celula};

/// Acima disto o detalhe avisa que ficou cortado (cada célula é uma escrita).
const MAX_LINHAS_DETALHE: usize = 1_000_000;

impl App {
    /// Duplo clique em (`r`, `c`) da aba atual. `true` quando era um valor de
    /// dinâmica e o clique foi tratado aqui (o editor da célula não abre).
    pub(in crate::app) fn detalhar_dinamica(&mut self, r: u32, c: u32) -> bool {
        let inicio = std::time::Instant::now();
        let tratado = self.detalhar_dinamica_sem_medir(r, c);
        if tratado {
            let ms = inicio.elapsed().as_millis();
            feathersheet_core::diario::anotar(trf!("[dinâmica] Detalhes: {ms} ms"));
        }
        tratado
    }

    fn detalhar_dinamica_sem_medir(&mut self, r: u32, c: u32) -> bool {
        let Some((destino, meta)) = localizar_na_celula(self, self.folha, r, c) else { return false };
        let Some(area) = retangulo_json(&meta["areaResultado"]) else { return false };
        // Rótulo ou cabeçalho (pelo mapa gravado): nem recalcula — antes
        // montava a dinâmica inteira só para descobrir isso.
        let rotulos = meta["mapa"]["rotulos"].as_u64().unwrap_or(1) as u32;
        let tipo = meta["mapa"]["linhas"].get((r - area.r0) as usize).and_then(|l| l[0].as_str());
        if c < area.c0 + rotulos || r == area.r0 || tipo == Some("c") { return false; }
        let e = match ler_estado(self, destino, &meta) {
            Ok(e) => e,
            Err(erro) => { self.status = erro; return true; }
        };
        let (linha, coluna) = ((r - area.r0) as usize, (c - area.c0) as usize);
        // Já calculada nesta sessão com a mesma configuração: só a varredura.
        if let Some(guardados) = avancado::caminhos_guardados(self, &e) {
            let caminho = guardados.0.get(linha).cloned().flatten();
            let ck = guardados.1.get(coluna).cloned().flatten();
            let (Some(caminho), Some(ck)) = (caminho, ck) else { return false };
            match avancado::linhas_do_caminho(self, &e, &caminho, &ck) {
                Ok(achadas) => self.criar_aba_de_detalhes(&e, &achadas),
                Err(erro) => self.status = erro,
            }
            avancado::renovar_caminhos(self, &e, guardados);
            return true;
        }
        let (resultado, achadas) = match avancado::detalhar(self, &e, linha, coluna) {
            Ok(Some(x)) => x,
            // Rótulo ou cabeçalho: segue o duplo clique de sempre.
            Ok(None) => return false,
            Err(erro) => { self.status = erro; return true; }
        };
        // A conta tem de ser a que está na planilha: o mesmo rótulo na
        // linha e o mesmo cabeçalho na coluna. Senão a origem mudou.
        let aba = self.workbook.aba(destino).map(|a| &a.sheet);
        let mesmo = |lr: usize, lc: usize| -> bool {
            let esperado = resultado.matriz.rows().get(lr).and_then(|l| l.get(lc)).map(to_text).unwrap_or_default();
            let na_planilha = aba.map(|s| s.get_raw(area.r0 + lr as u32, area.c0 + lc as u32)).unwrap_or_default();
            esperado.trim() == na_planilha.trim_start_matches('\'').trim()
        };
        let confere = (0..resultado.colunas_de_rotulo).all(|j| mesmo(linha, j)) && mesmo(0, coluna);
        if !confere {
            self.status = tr!("Não deu para mostrar os detalhes desta célula. Atualize a dinâmica e tente de novo.").into();
            return true;
        }
        self.criar_aba_de_detalhes(&e, &achadas);
        avancado::guardar_caminhos(self, &e, &resultado);
        true
    }

    fn criar_aba_de_detalhes(&mut self, e: &Estado, achadas: &[usize]) {
        let nomes = self.workbook.nomes();
        let nome = (1..).map(|n| trf!("Detalhes{n}"))
            .find(|n| !nomes.iter().any(|x| x.eq_ignore_ascii_case(n)))
            .expect("um nome livre sempre existe");
        let cortado = achadas.len() > MAX_LINHAS_DETALHE;
        let linhas = &achadas[..achadas.len().min(MAX_LINHAS_DETALHE)];
        let largura = e.origem.c1 - e.origem.c0 + 1;
        // Coluna inteira de uma vez, como na abertura do .fshz: números e
        // textos direto nos blocos (era 1 escrita por célula — 1,5 s para
        // 125 mil linhas × 13 colunas, 17/10/2026). Só o raro (verdadeiro/
        // falso, erro, texto que começa com "=") passa célula a célula.
        let altura = linhas.len() + 1;
        let bytes = altura.div_ceil(8);
        let mut colunas: Vec<ColunaDetalhe> = Vec::with_capacity(largura as usize);
        let mut avulsas: Vec<(u32, u32, String)> = Vec::new();
        let origem_da = |k: usize| if k == 0 { e.origem.r0 } else { e.origem.r0 + linhas[k - 1] as u32 };
        for j in 0..largura {
            let mut col = ColunaDetalhe {
                numeros: vec![0.0; altura], com_numero: vec![0; bytes],
                ids: vec![0; altura], com_texto: vec![0; bytes],
                dicionario: Vec::new(), ja_visto: std::collections::HashMap::new(),
            };
            for k in 0..altura {
                let marcar = |m: &mut Vec<u8>| m[k >> 3] |= 1 << (k & 7);
                match self.workbook.valor(e.folha_origem, origem_da(k), e.origem.c0 + j) {
                    Value::Empty => {}
                    Value::Number(x) => { col.numeros[k] = x; marcar(&mut col.com_numero); }
                    Value::Text(t) if !t.is_empty() && !t.starts_with('=') => {
                        let proximo = col.dicionario.len() as i32;
                        let id = *col.ja_visto.entry(t).or_insert_with_key(|t| {
                            col.dicionario.push(t.clone());
                            proximo
                        });
                        col.ids[k] = id;
                        marcar(&mut col.com_texto);
                    }
                    outro => avulsas.push((k as u32, j, texto_celula(&outro))),
                }
            }
            colunas.push(col);
        }
        let indice = self.workbook.criar_aba(nome.clone());
        self.workbook.editar(indice, |sheet| {
            for (j, col) in colunas.iter().enumerate() {
                if col.com_numero.iter().any(|&b| b != 0) {
                    sheet.carregar_coluna_numeros(j as u32, &col.numeros, &col.com_numero);
                }
                if !col.dicionario.is_empty() {
                    sheet.carregar_coluna_textos(j as u32, &col.ids, &col.com_texto, &col.dicionario);
                }
            }
            sheet.terminar_carga();
            for (r, c, t) in &avulsas {
                sheet.set_cell(*r, *c, t);
            }
        });
        self.alterado = true;
        self.folha = indice;
        self.linha_inicio = 0;
        self.coluna_inicio = 0;
        self.linha = 0;
        self.coluna = 0;
        self.fim_selecao = (linhas.len() as u32, largura.saturating_sub(1));
        self.endereco = cell_name(0, 0);
        self.status = if cortado {
            trf!("Detalhes em {nome}: as primeiras {} de {} linhas da origem.", linhas.len(), achadas.len())
        } else {
            trf!("Detalhes em {nome}: {} linha(s) da origem.", linhas.len())
        };
    }
}

/// Uma coluna da aba de Detalhes montada antes de gravar.
struct ColunaDetalhe {
    numeros: Vec<f64>,
    com_numero: Vec<u8>,
    ids: Vec<i32>,
    com_texto: Vec<u8>,
    dicionario: Vec<String>,
    ja_visto: std::collections::HashMap<String, i32>,
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn duplo_clique_no_valor_abre_as_linhas_da_origem() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor"); s.set_cell(0, 2, "Obs");
            s.set_cell(1, 0, "Norte"); s.set_cell(1, 1, "10"); s.set_cell(1, 2, "texto");
            s.set_cell(2, 0, "Sul"); s.set_cell(2, 1, "30");
            s.set_cell(3, 0, "Norte"); s.set_cell(3, 1, "20"); s.set_cell(3, 2, "VERDADEIRO");
        });
        app.fim_selecao = (3, 2);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        let aba = app.folha;
        let s = &app.workbook.aba(aba).unwrap().sheet;
        // Acha a linha "Norte" do resultado.
        let r = (0..10).find(|&r| s.get_raw(r, 0) == "Norte").expect("linha Norte");
        assert!(app.detalhar_dinamica(r, 1));
        assert_ne!(app.folha, aba, "abriu uma aba nova");
        let d = &app.workbook.aba(app.folha).unwrap().sheet;
        assert_eq!(d.get_raw(0, 0), "Região");
        assert_eq!((d.get_raw(1, 1), d.get_raw(2, 1), d.get_raw(3, 0)), ("10".into(), "20".into(), String::new()));
        // Coluna misturada: texto pelo dicionário, booleano célula a célula.
        assert_eq!((d.get_raw(0, 2), d.get_raw(1, 2)), ("Obs".into(), "texto".into()));
        assert_eq!(app.workbook.valor(app.folha, 2, 2), Value::Bool(true));
        assert_eq!(app.workbook.valor(app.folha, 2, 1), Value::Number(20.0));
        // O rótulo não detalha.
        app.folha = aba;
        assert!(!app.detalhar_dinamica(r, 0));
    }
}
