//! Intervalos editáveis e sugestões de configuração de tabela dinâmica.
use super::*;
use feathersheet_core::dinamica::campos;

fn intervalo(texto: &str) -> Option<Rect> {
    feathersheet_core::celulas::refs::parse_range_ref(texto).map(|r| r.normalized())
}

/// O texto da caixa de origem aponta para uma área com cabeçalho e ao menos uma linha.
pub(super) fn texto_valido(w: &Workbook, texto: &str, folha_atual: usize) -> bool {
    resolver(w, texto, folha_atual).is_some_and(|(_, r)| r.r0 < r.r1)
}

/// A origem digitada: `A1:D9` (na aba de origem atual), `Dados!A1:D9`, `'Minha aba'!A:D`
/// ou o nome de uma tabela/intervalo nomeado (`Vendas`). Devolve a aba e o retângulo sem aba.
fn resolver(w: &Workbook, texto: &str, folha_atual: usize) -> Option<(usize, Rect)> {
    let t = texto.trim().trim_start_matches('=');
    if let Some((aba, area)) = t.rsplit_once('!') {
        let aba = aba.trim();
        let aba = aba.strip_prefix('\'').and_then(|a| a.strip_suffix('\'')).map(|a| a.replace("''", "'")).unwrap_or_else(|| aba.to_owned());
        return Some((w.indice_por_nome(&aba)?, intervalo(area)?));
    }
    if let Some(r) = intervalo(t) { return Some((folha_atual, r)); }
    let n = w.nomeados().into_iter().find(|n| n.nome.eq_ignore_ascii_case(t))?;
    let folha = match &n.area.sheet { Some(a) => w.indice_por_nome(a)?, None => folha_atual };
    Some((folha, Rect::new(n.area.r0, n.area.c0, n.area.r1, n.area.c1).normalized()))
}

impl App {
    /// OK do primeiro passo: usa o intervalo digitado, se mudou.
    pub(super) fn aplicar_texto_antes_de_criar(&mut self) -> bool {
        let Some(e) = self.dinamica.as_ref() else { return false };
        match resolver(&self.workbook, &e.origem_texto, e.folha_origem) {
            None => {
                self.status = tr!("Origem inválida. Use, por exemplo, A1:D100, Dados!A1:D100 ou o nome de uma tabela.").into();
                false
            }
            Some(nova) if nova != (e.folha_origem, e.origem.clone()) => {
                self.aplicar_nova_origem_dinamica();
                self.dinamica.as_ref().is_some_and(|e| (e.folha_origem, e.origem.clone()) == nova)
            }
            Some(_) => true,
        }
    }

    pub(super) fn aplicar_nova_origem_dinamica(&mut self) {
        let Some(e) = self.dinamica.as_ref() else { return };
        let Some((folha, nova)) = resolver(&self.workbook, &e.origem_texto, e.folha_origem) else {
            self.status = tr!("Origem inválida. Use, por exemplo, A1:D100, Dados!A1:D100 ou o nome de uma tabela.").into();
            return;
        };
        if nova.r0 >= nova.r1 {
            self.status = tr!("A origem deve ter cabeçalho e pelo menos uma linha de dados.").into();
            return;
        }
        if nova == e.origem && folha == e.folha_origem {
            self.status = tr!("A origem já corresponde ao intervalo informado.").into();
            return;
        }
        let texto = match self.workbook.aba(folha) {
            Some(a) if folha != self.folha => format!("{}!{}", feathersheet_core::celulas::refs::quote_sheet(&a.sheet.name), endereco_area(&nova)),
            _ => endereco_area(&nova),
        };
        let campos = campos::campos(nova.clone(), |r,c| self.workbook.valor(folha,r,c));
        let col_medida = campos.iter().enumerate().rev().find(|(_,c)| c.numerico)
            .map(|(i,_)| nova.c0 as usize + i).unwrap_or(nova.c1 as usize);
        if let Some(e) = self.dinamica.as_mut() {
            e.origem = nova.clone();
            e.folha_origem = folha;
            e.origem_texto = texto;
            e.nomes = campos.into_iter().map(|c| c.nome).collect();
            e.linhas = vec![nova.c0 as usize];
            e.colunas.clear();
            e.valores = vec![MedidaDinamica{coluna:col_medida,op:AgregacaoOp::Soma}];
            e.calculados.clear();
            e.filtros.clear();
            e.recolhidos.clear();
            e.grupos.clear();
            e.campo_filtro = None;
            e.valores_filtro.clear();
            e.filtro_cortado = false;
        }
        self.status = tr!("Intervalo aplicado. Campos e filtros anteriores foram redefinidos.").into();
        if self.dinamica.as_ref().is_some_and(|e| e.sugestoes.is_some()) { self.calcular_sugestoes_dinamica(); }
    }

    /// As dinâmicas recomendadas da origem do painel aberto.
    pub(super) fn calcular_sugestoes_dinamica(&mut self) {
        let Some(e) = self.dinamica.as_ref() else { return };
        let (origem, folha) = (e.origem.clone(), e.folha_origem);
        let lista = campos::sugestoes(origem, |r, c| self.workbook.valor(folha, r, c));
        if let Some(e) = self.dinamica.as_mut() { e.sugestoes = Some(lista); }
    }

    pub(super) fn preset_dinamica(&mut self, id: u8) {
        let Some(e) = self.dinamica.as_mut() else { return };
        let col0 = e.origem.c0 as usize;
        let largura = e.origem.c1 as usize - col0 + 1;
        if largura < 2 {
            self.status = tr!("É necessário ter pelo menos dois campos para as sugestões.").into();
            return;
        }
        let medida = e.valores.first().map(|v| v.coluna).unwrap_or(e.origem.c1 as usize);
        e.linhas = vec![col0];
        e.colunas.clear();
        e.valores = vec![MedidaDinamica {
            coluna: medida,
            op: if id == 1 { AgregacaoOp::Media } else { AgregacaoOp::Soma },
        }];
        if id == 2 {
            let candidato = (col0+1..=e.origem.c1 as usize)
                .find(|&col| col != medida)
                .unwrap_or(col0+1);
            e.colunas = vec![candidato];
        }
        self.status = tr!("Sugestão selecionada. Revise os campos e confirme a dinâmica.").into();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn leitura_de_intervalo_com_coordenadas_absolutas() {
        assert_eq!(intervalo("B2:D20"),Some(Rect::new(1,1,19,3)));
        assert_eq!(intervalo("D20:B2"),Some(Rect::new(1,1,19,3)));
        assert!(intervalo("B0:D1").is_none());
        assert!(intervalo("texto").is_none());
    }

    #[test]
    fn troca_de_origem_redefine_campos_sem_tocar_nas_celulas() {
        let mut app = App::default();
        app.workbook.editar(0,|sheet|{
            sheet.set_cell(0,0,"A");sheet.set_cell(0,1,"B");
            sheet.set_cell(1,0,"x");sheet.set_cell(1,1,"12");
            sheet.set_cell(2,0,"y");sheet.set_cell(2,1,"15");
        });
        app.fim_selecao = (1,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::OrigemTexto("A1:B3".into()));
        app.evento_dinamica(Evento::OrigemAplicar);
        assert_eq!(app.dinamica.as_ref().unwrap().origem.r1,2);
        assert_eq!(app.workbook.valor(0,2,1),Value::Number(15.));
    }

    #[test]
    fn origem_aceita_outra_aba_aspas_e_nome_de_tabela() {
        let mut w = Workbook::default();
        let dados = w.criar_aba("Meus dados");
        w.definir_nome("Vendas", Rect::na_aba(0, 0, 9, 2, "Meus dados".to_owned()));
        assert_eq!(resolver(&w, "B2:D20", 0), Some((0, Rect::new(1,1,19,3))));
        assert_eq!(resolver(&w, "'Meus dados'!A1:C10", 0), Some((dados, Rect::new(0,0,9,2))));
        assert_eq!(resolver(&w, "=meus dados!A1:C10", 0), Some((dados, Rect::new(0,0,9,2))));
        assert_eq!(resolver(&w, "vendas", 0), Some((dados, Rect::new(0,0,9,2))));
        assert_eq!(resolver(&w, "Sumiu!A1:B2", 0), None);
        assert_eq!(resolver(&w, "NaoExiste", 0), None);
    }
}
