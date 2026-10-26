//! Gráfico dinâmico (básico, 16/10/2026, meta da v0.1.0): Inserir ▸ Gráfico
//! dinâmico cria um gráfico ligado a uma tabela dinâmica. Com a seleção numa
//! dinâmica, o seletor de tipo abre com a área de dados dela (sem a linha e
//! as colunas de Total Geral); fora de uma, cria a dinâmica primeiro e o
//! gráfico vem junto. O gráfico guarda `pivotSource` = id da dinâmica e, a
//! cada atualização dela (filtro, campo, Atualizar), segue a área nova.
use super::*;
use serde_json::{json, Value as Json};

/// A área do gráfico dentro do resultado: cabeçalho e itens, sem a linha e
/// as colunas de "Total Geral" (o gráfico dobraria a escala com elas).
pub(super) fn area_do_grafico(app: &App, destino: usize, meta: &Json) -> Option<Rect> {
    let area = retangulo_json(&meta["areaResultado"])?;
    let sheet = &app.workbook.aba(destino)?.sheet;
    let tipos: Vec<String> = meta["mapa"]["linhas"].as_array()
        .map(|l| l.iter().map(|x| x[0].as_str().unwrap_or_default().to_owned()).collect())
        .unwrap_or_default();
    let mut r1 = area.r1;
    while r1 > area.r0 {
        let total = match tipos.get((r1 - area.r0) as usize) {
            Some(t) => t == "t",
            None => sheet.get_raw(r1, area.c0).starts_with("Total Geral"),
        };
        if !total { break; }
        r1 -= 1;
    }
    let mut c1 = area.c1;
    while c1 > area.c0 && sheet.get_raw(area.r0, c1).starts_with("Total Geral") {
        c1 -= 1;
    }
    (r1 > area.r0).then(|| Rect::new(area.r0, area.c0, r1, c1))
}

impl App {
    pub(super) fn inserir_grafico_dinamico(&mut self) -> Task<Msg> {
        let Some((destino, meta)) = super::edicao::localizar_na_celula(self, self.folha, self.linha, self.coluna) else {
            // Fora de uma dinâmica: cria a dinâmica e o gráfico vem junto.
            let tarefa = self.evento_dinamica_base(Evento::Abrir);
            if let Some(e) = &mut self.dinamica {
                e.com_grafico = true;
            }
            return tarefa;
        };
        let Some(area) = area_do_grafico(self, destino, &meta) else {
            self.status = tr!("A tabela dinâmica ainda não tem itens para o gráfico.").into();
            return Task::none();
        };
        let id = meta["id"].as_str().unwrap_or_default().to_owned();
        // O seletor de tipo de gráfico usa a seleção como intervalo.
        self.folha = destino;
        self.linha = area.r0;
        self.coluna = area.c0;
        self.fim_selecao = (area.r1, area.c1);
        self.graficos.seletor_aberto = false;
        let tarefa = self.evento_grafico(crate::app::graficos::Evento::Abrir);
        self.graficos.dinamica = Some(id);
        tarefa
    }

    /// Depois de atualizar a dinâmica `id` (resultado em `destino`): os
    /// gráficos dinâmicos dela, em qualquer aba, passam para a área nova.
    pub(super) fn seguir_graficos_dinamicos(&mut self, destino: usize, meta: &Json) {
        let Some(id) = meta["id"].as_str() else { return };
        let Some(area) = area_do_grafico(self, destino, meta) else { return };
        let Some(nome_destino) = self.workbook.aba(destino).map(|a| a.sheet.name.clone()) else { return };
        for i in 0..self.workbook.len() {
            let Some(aba) = self.workbook.aba_mut(i) else { continue };
            let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) else { continue };
            let ligados: Vec<String> = lista.iter()
                .filter(|v| v["pivotSource"].as_str() == Some(id))
                .filter_map(|v| v["id"].as_str().map(str::to_owned))
                .collect();
            for grafico in ligados {
                let mut novo = json!({"id": grafico, "range": area});
                if i != destino {
                    novo["sourceSheet"] = json!(nome_destino);
                }
                if let Ok(texto) = feathersheet_io::graficos_salvos::mesclado(&aba.sheet, &grafico, novo) {
                    aba.sheet.alterar_graficos(Some(texto), false);
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grafico_dinamico_pega_a_area_sem_o_total_e_segue_a_dinamica() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor");
            s.set_cell(1, 0, "Norte"); s.set_cell(1, 1, "10");
            s.set_cell(2, 0, "Sul"); s.set_cell(2, 1, "30");
        });
        app.fim_selecao = (2, 1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        app.dinamica = None;
        let destino = app.folha;
        let (_, meta) = super::edicao::localizar_na_celula(&app, destino, app.linha, app.coluna).expect("na dinâmica");
        let area = area_do_grafico(&app, destino, &meta).expect("área");
        // Cabeçalho + Norte + Sul, sem "Total Geral".
        assert_eq!((area.r1 - area.r0, area.c1 - area.c0), (2, 1));
        let _ = app.inserir_grafico_dinamico();
        assert_eq!(app.graficos.dinamica.as_deref(), meta["id"].as_str());
    }

    /// O caso do usuário (18/10): painel de campos aberto, quatro campos em
    /// Linhas e Desvio padrão; Gráfico dinâmico e escolher o tipo tem que
    /// pôr o gráfico na planilha.
    #[test]
    fn grafico_dinamico_com_painel_aberto_e_varios_campos_entra_na_planilha() {
        use feathersheet_core::dinamica::pivot::AgregacaoOp;
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            for (c, t) in ["Ramal", "Nome", "Ini", "Fim", "Nivel"].iter().enumerate() { s.set_cell(0, c as u32, t); }
            for r in 1..=60u32 {
                s.set_cell(r, 0, &format!("R{}", r % 2));
                s.set_cell(r, 1, &format!("N{}", r % 5));
                s.set_cell(r, 2, &(r % 3).to_string());
                s.set_cell(r, 3, &(r % 3 + 10).to_string());
                s.set_cell(r, 4, &format!("{}", r % 7));
            }
        });
        app.fim_selecao = (60, 4);
        app.evento_dinamica(Evento::Abrir);
        app.evento_dinamica(Evento::Criar);
        let e = app.dinamica.as_mut().expect("painel aberto");
        e.linhas = vec![0, 1, 2, 3];
        e.valores = vec![MedidaDinamica { coluna: 4, op: AgregacaoOp::DesvioPadrao }];
        let estado = app.dinamica.clone().unwrap();
        let destino = app.folha;
        app.aplicar_estado_na_dinamica(destino, &estado).expect("aplica");
        assert!(app.dinamica.is_some(), "painel continua aberto");
        let _ = app.inserir_grafico_dinamico();
        assert!(app.graficos.seletor_aberto, "a galeria de tipos abre");
        let _ = app.update(Msg::Grafico(crate::app::graficos::Evento::Tipo(crate::app::graficos::TipoUi::Colunas)));
        let aba = app.workbook.aba(app.folha).unwrap();
        let lista = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
        assert_eq!(lista.len(), 1, "gráfico na planilha; status: {}", app.status);
    }
}
