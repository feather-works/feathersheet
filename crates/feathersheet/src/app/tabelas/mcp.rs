//! `sheet_tabela` do MCP ao vivo: as mesmas funções dos botões da faixa Tabela (criar, renomear, estilo, totais,
//! redimensionar, converter), cada uma com o seu Ctrl+Z.
use super::super::*;
use super::{converter, estilo, listar, redimensionar, renomear, Estilo};
use serde_json::Value as Json;

impl App {
    pub(in crate::app) fn tabela_pelo_mcp(&mut self, aba: usize, args: &Json,
        area: Option<Rect>) -> Result<String, String>
    {
        let texto = |k: &str| args.get(k).and_then(Json::as_str).map(str::trim).filter(|s| !s.is_empty());
        let acao = texto("acao").unwrap_or("listar");
        let tabelas = listar(&self.workbook.aba(aba).ok_or("Aba inexistente.")?.sheet)?;
        let tabelas: Vec<Json> = tabelas.into_iter().filter(|t| t["soFiltro"] != true).collect();
        let rect = |t: &Json| { let a = &t["area"]; let n = |k: &str| a[k].as_u64().unwrap_or(0) as u32;
            Rect::new(n("r0"), n("c0"), n("r1"), n("c1")) };
        if acao == "listar" {
            if tabelas.is_empty() { return Ok("Nenhuma tabela nesta aba.".into()); }
            return Ok(tabelas.iter().map(|t| {
                let r = rect(t);
                format!("{}: {}:{}{}{}", t["nome"].as_str().unwrap_or("?"), cell_name(r.r0, r.c0), cell_name(r.r1, r.c1),
                    if t["mostrarTotais"] == true { " · com totais" } else { "" },
                    t["estilo"]["tema"].as_str().map(|x| format!(" · tema {x}")).unwrap_or_default())
            }).collect::<Vec<_>>().join("\n"));
        }
        if acao == "criar" {
            let r = area.ok_or("Falta `intervalo`.")?;
            let nome = match texto("nome") { Some(n) => n.to_string(), None => tabelas::nome_sugerido(&self.workbook)? };
            let cabecalho = args.get("cabecalho").and_then(Json::as_bool).unwrap_or(true);
            let vazia = (r.r0..=r.r1).all(|l| (r.c0..=r.c1).all(|c| self.workbook.aba(aba).is_some_and(|a| a.sheet.get_raw(l, c).is_empty())));
            tabelas::criar(&mut self.workbook, aba, r.clone(), &nome, cabecalho, vazia)?;
            self.alterado = true;
            return Ok(format!("Tabela {nome} criada em {}:{}", cell_name(r.r0, r.c0), cell_name(r.r1, r.c1)));
        }
        let alvo = texto("tabela").ok_or("Falta `tabela` (o nome; veja acao listar).")?;
        let t = tabelas.iter().find(|t| t["nome"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(alvo)))
            .ok_or_else(|| format!("Tabela {alvo} não está nesta aba (acao listar mostra as que há)."))?;
        let id = t["id"].as_str().unwrap_or_default().to_string();
        let r = rect(t);
        let feito = match acao {
            "renomear" => {
                let novo = texto("novo_nome").ok_or("Falta `novo_nome`.")?;
                renomear(&mut self.workbook, aba, &id, novo)?;
                format!("Tabela {alvo} agora se chama {novo}")
            }
            "estilo" => {
                let mut mudou = Vec::new();
                for (k, f) in [("faixas", Estilo::Faixas as fn(bool) -> Estilo), ("faixas_colunas", Estilo::FaixasColuna), ("borda", Estilo::Borda)] {
                    if let Some(v) = args.get(k).and_then(Json::as_bool) { estilo(&mut self.workbook, aba, &id, f(v))?; mudou.push(k); }
                }
                if let Some(tema) = texto("tema") { estilo(&mut self.workbook, aba, &id, Estilo::Tema(tema.into()))?; mudou.push("tema"); }
                if let Some(v) = args.get("totais").and_then(Json::as_bool) {
                    super::totais::alternar_totais(&mut self.workbook, aba, &id, v)?; mudou.push("totais");
                }
                if mudou.is_empty() { return Err("Nada para mudar: faixas, faixas_colunas, borda, tema ou totais.".into()); }
                format!("Tabela {alvo}: {} alterado", mudou.join(", "))
            }
            "total" => {
                let funcao = texto("funcao").ok_or("Falta `funcao` (soma, media, contagem…).")?;
                if !super::totais::CONTAS.iter().any(|(k, _, _)| *k == funcao) {
                    return Err(format!("Função de total desconhecida: {funcao}. Use {}.",
                        super::totais::CONTAS.iter().map(|(k, _, _)| *k).collect::<Vec<_>>().join(", ")));
                }
                let col = texto("coluna").ok_or("Falta `coluna` (letra ou título).")?;
                let titulo = |c: u32| self.workbook.aba(aba).map(|a| a.sheet.get_raw(r.r0, c)).unwrap_or_default();
                let c = (r.c0..=r.c1).find(|&c| titulo(c).trim().eq_ignore_ascii_case(col))
                    .or_else(|| feathersheet_core::celulas::refs::parse_cell_ref(&format!("{col}1")).map(|(_, c)| c).filter(|c| (r.c0..=r.c1).contains(c)))
                    .ok_or_else(|| format!("Coluna {col} não está na tabela {alvo}."))?;
                // Como o botão: liga a linha (se preciso) e grava a conta num Ctrl+Z.
                let ligada = t["mostrarTotais"] == true;
                if let Some(a) = self.workbook.aba_mut(aba) { a.sheet.abrir_transacao(); }
                let res = (if ligada { Ok(()) } else { super::totais::alternar_totais(&mut self.workbook, aba, &id, true) })
                    .and_then(|_| super::totais::definir_total(&mut self.workbook, aba, &id, c - r.c0, funcao));
                if let Some(a) = self.workbook.aba_mut(aba) { if res.is_ok() { a.sheet.fechar_transacao(); } else { a.sheet.reverter_transacao(); } }
                res?;
                format!("Total {funcao} na coluna {}", feathersheet_core::celulas::refs::col_name(c))
            }
            "redimensionar" => {
                let novo = area.ok_or("Falta `intervalo`.")?;
                redimensionar(&mut self.workbook, aba, &id, novo.clone())?;
                format!("Tabela {alvo} agora em {}:{}", cell_name(novo.r0, novo.c0), cell_name(novo.r1, novo.c1))
            }
            "converter" => { converter(&mut self.workbook, aba, &id)?; format!("Tabela {alvo} virou intervalo comum") }
            outra => return Err(format!("Ação desconhecida: {outra}. Use listar, criar, renomear, estilo, total, redimensionar ou converter.")),
        };
        self.alterado = true;
        if aba == self.folha { self.trocar_selecao(self.linha, self.coluna); }
        Ok(feito)
    }
}
