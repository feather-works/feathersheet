//! Leitor de `.xlsm`: os dados abrem como os de um `.xlsx` e as macros VBA
//! são convertidas para JavaScript pelo `feathersheet-vba`. A conversão fica
//! esperando: só entra na biblioteca de macros (Alt+F11) quando a pessoa
//! pede (Macros → Importar VBA), e nada roda sozinho — nem o que no Excel
//! era `Workbook_Open`.
use super::*;
use feathersheet_vba::Conversao;

/// A conversão de um `.xlsm` aberto, com o nome do arquivo de origem.
#[derive(Debug, Clone)]
pub(super) struct VbaPendente {
    pub arquivo: String,
    pub conversao: Conversao,
}

/// Nome de módulo aceito pela biblioteca: letras, números e `_`.
fn nome_modulo(texto: &str) -> String {
    let limpo: String = texto.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let limpo = limpo.trim_matches('_').to_string();
    if limpo.is_empty() { tr!("Macro").into() } else { limpo }
}

/// Um nome que a biblioteca aceita: até 128 letras e sem repetir outro,
/// sem diferenciar maiúsculas (inclusive acentuadas, como ela confere).
fn livre(catalogo: &feather_macros::Catalogo, base: &str, usados: &[String]) -> String {
    let base: String = base.chars().take(100).collect();
    let ocupado = |nome: &str| {
        let chave = nome.to_lowercase();
        catalogo.macros.iter().any(|m| m.name.trim().to_lowercase() == chave)
            || usados.iter().any(|u| u.to_lowercase() == chave)
    };
    let mut nome = base.clone();
    let mut n = 2;
    while ocupado(&nome) {
        nome = format!("{base}_{n}");
        n += 1;
    }
    nome
}

/// Os módulos que a conversão vira: um por macro, um por evento (com aviso
/// de que não roda sozinho) e um com as pendências, se houver.
pub(super) fn modulos_da_conversao(
    catalogo: &feather_macros::Catalogo, arquivo: &str, c: &Conversao,
) -> Vec<feather_macros::Macro> {
    let mut usados: Vec<String> = Vec::new();
    let mut saida = Vec::new();
    let mut acrescentar = |base: String, code: String, saida: &mut Vec<feather_macros::Macro>| {
        let name = livre(catalogo, &nome_modulo(&base), &usados);
        usados.push(name.clone());
        saida.push(feather_macros::Macro { name, code, auto: false });
    };
    for m in &c.macros {
        let code = format!("// Convertida do VBA de {arquivo} (módulo {}).\n{}", m.modulo, m.codigo);
        acrescentar(m.nome.clone(), code, &mut saida);
    }
    for e in &c.eventos {
        let onde = e.aba.as_deref().map_or("da pasta".to_string(), |a| format!("da aba {a}"));
        let code = format!(
            "// Evento {} {onde} (era {} no VBA de {arquivo}).\n// {}\n{}",
            e.evento, e.origem, match e.evento {
                "aoMudar" => tr!("Roda sozinho quando uma célula da aba muda (valores.intervalo é o endereço mudado)."),
                "aoAtivar" => tr!("Roda sozinho quando a aba é aberta."),
                _ => tr!("Não roda sozinho ao abrir o arquivo: execute à mão."),
            }, e.codigo);
        acrescentar(format!("Evento_{}_{}", e.evento, e.aba.as_deref().unwrap_or(tr!("Pasta"))), code, &mut saida);
    }
    if !c.pendencias.is_empty() {
        let mut code = format!("// O que do VBA de {arquivo} não foi convertido (ou pode agir diferente).\n");
        for p in &c.pendencias {
            let tipo = if p.aviso { "aviso" } else { tr!("não convertido") };
            // Bloco de várias linhas (With … End With): todas comentadas.
            let vba = p.vba.trim().lines().map(|l| format!("//     {l}")).collect::<Vec<_>>().join("\n");
            code.push_str(&format!("// [{tipo}] {} linha {}: {}\n{vba}\n", p.modulo, p.linha, p.motivo));
        }
        acrescentar(format!("VBA_pendencias_{arquivo}"), code, &mut saida);
    }
    saida
}

/// Se o módulo foi convertido do VBA de `arquivo` (a 1ª linha diz).
fn veio_do_arquivo(m: &feather_macros::Macro, arquivo: &str) -> bool {
    let linha = m.code.lines().next().unwrap_or("");
    [" (", ").", tr!(" não")].iter().any(|fim| linha.contains(&format!("VBA de {arquivo}{fim}")))
}

fn resumo(c: &Conversao) -> String {
    trf!("{} macro(s), {} evento(s), {} formulário(s), {} pendência(s)", c.macros.len(), c.eventos.len(), c.formularios.len(), c.pendencias.len())
}

/// O formulário do VBA no modelo dos formulários das macros. `modulo` é o nome que o módulo
/// com o código do formulário recebeu na biblioteca: cada controle chama `modulo.função`.
fn formulario_da_conversao(f: &feathersheet_vba::gerar::FormularioConvertido, modulo: Option<&str>) -> feathersheet_macros::formularios::Formulario {
    use feathersheet_macros::formularios::{Controle, Formulario, TipoControle as T, MAX_CONTROLES, MAX_IMAGEM};
    use feathersheet_vba::formulario::TipoVba as V;
    use base64::Engine;
    let d = &f.desenho;
    let mut form = Formulario::novo(&d.nome);
    form.titulo = d.titulo.clone();
    (form.largura, form.altura) = (d.largura.max(120.0), d.altura.max(80.0));
    form.fundo = d.fundo.clone();
    for c in d.controles.iter().take(MAX_CONTROLES) {
        let tipo = match c.tipo {
            V::Rotulo => T::Rotulo,
            V::Texto if c.senha => T::Senha,
            V::Texto => T::Texto,
            V::Lista => T::Lista,
            V::Listagem => T::Listagem,
            V::Caixa | V::Alternar => T::Caixa,
            V::Opcao => T::Opcao,
            V::Botao => T::Botao,
            V::Moldura => T::Moldura,
            V::Imagem => T::Imagem,
            V::Outro(_) => continue,
        };
        let marcado = matches!(c.valor.trim().to_ascii_lowercase().as_str(), "1" | "-1" | "true" | "verdadeiro");
        let imagem = (tipo == T::Imagem && (c.figura.starts_with(&[0x89, b'P', b'N', b'G']) || c.figura.starts_with(&[0xFF, 0xD8])))
            .then(|| base64::engine::general_purpose::STANDARD.encode(&c.figura))
            .filter(|b| b.len() <= MAX_IMAGEM)
            .unwrap_or_default();
        form.controles.push(Controle {
            tipo,
            // As opções sem GroupName formam um grupo só, como no VBA.
            nome: if tipo == T::Opcao { if c.grupo.is_empty() { tr!("Opcoes").into() } else { c.grupo.clone() } } else { c.nome.clone() },
            texto: c.texto.clone(),
            x: c.x, y: c.y,
            largura: c.largura.max(16.0), altura: c.altura.max(12.0),
            opcoes: c.itens.clone(),
            livre: tipo == T::Lista && !c.so_lista,
            multipla: c.multipla,
            fonte: c.fonte.clone(),
            valor: match tipo {
                T::Caixa | T::Opcao => if marcado { "1".into() } else { String::new() },
                _ => c.valor.clone(),
            },
            macro_nome: modulo.and_then(|m| f.macros.iter().find(|(ctl, _)| *ctl == c.nome).map(|(_, js)| format!("{m}.{js}"))),
            padrao: c.padrao, cancelar: c.cancelar,
            desativado: c.desativado, oculto: c.oculto,
            fundo: c.fundo.clone(), cor: c.cor.clone(),
            // Pontos do VBA → px.
            letra: c.letra.map(|pt| (pt * 4.0 / 3.0).round()),
            negrito: c.negrito, italico: c.italico,
            alinhar: c.alinhar.clone(),
            max: c.max, multilinha: c.multilinha,
            dica: c.dica.clone(),
            imagem,
            ..Default::default()
        });
    }
    form
}

/// A macro que a forma chama, no nome que o módulo convertido recebe: a do
/// próprio desenho (`macro=`) ou a do `.OnAction` do VBA.
fn macro_da_forma(f: &feathersheet_io::xlsx_read::Forma, c: Option<&Conversao>) -> Option<String> {
    let alvo = f.macro_.clone().or_else(|| c?.botoes.iter()
        .find(|b| b.forma.eq_ignore_ascii_case(&f.nome)).map(|b| b.macro_.clone()))?;
    let alvo = alvo.rsplit(['!', '.']).next().unwrap_or(&alvo).to_string();
    // Com dois módulos de mesmo nome, a conversão chama "Modulo.Macro".
    let nome = c.and_then(|c| c.macros.iter().map(|m| &m.nome)
        .find(|n| n.eq_ignore_ascii_case(&alvo) || n.rsplit('.').next().is_some_and(|x| x.eq_ignore_ascii_case(&alvo))))
        .cloned().unwrap_or(alvo);
    Some(nome_modulo(&nome))
}

impl App {
    /// As formas com texto do `.xlsm` (os retângulos que o Excel usa como
    /// botão) viram botões de macro na mesma aba, posição, texto e cor. A
    /// macro vem do desenho ou do `.OnAction` do VBA; sem ela, o botão fica
    /// sem macro e a pessoa escolhe em "Editar botão…".
    pub(super) fn botoes_das_formas(&mut self, bytes: &[u8], c: Option<&Conversao>) {
        use crate::grade::modelo::{inicio_x, inicio_y};
        let formas = feathersheet_io::xlsx_read::ler_formas(bytes);
        let mut por_aba: std::collections::BTreeMap<usize, Vec<serde_json::Value>> = Default::default();
        for (n, f) in formas.iter().filter(|f| !f.texto.is_empty()).enumerate() {
            if f.aba >= self.workbook.len() { continue; }
            let x = inicio_x(&self.workbook, f.aba, f.coluna) + f.dx as f32;
            let y = inicio_y(&self.workbook, f.aba, f.linha) + f.dy as f32;
            let mut item = serde_json::json!({
                "id": format!("xlsx-forma-{n}"),
                "label": f.texto,
                "macroName": macro_da_forma(f, c).unwrap_or_default(),
                "position": { "x": x.round(), "y": y.round(),
                    "width": f.largura.max(40.0).round(), "height": f.altura.max(24.0).round() },
            });
            if let Some(cor) = &f.cor { item["color"] = serde_json::json!(cor); }
            por_aba.entry(f.aba).or_default().push(item);
        }
        for (aba, itens) in por_aba {
            let json = serde_json::Value::Array(itens.into_iter().take(128).collect()).to_string();
            if let Some(a) = self.workbook.aba_mut(aba) { a.sheet.bt = Some(json); }
        }
    }

    /// Depois de abrir um `.xlsm`: converte o VBA e avisa, sem importar.
    pub(super) fn ler_vba_do_xlsm(&mut self, path: &Path) {
        self.vba_pendente = None;
        let arquivo = path.file_stem().and_then(|s| s.to_str()).unwrap_or("planilha").to_string();
        let Ok(bytes) = std::fs::read(path) else { return };
        let conversao = feathersheet_vba::converter_xlsm(&bytes, &arquivo);
        self.botoes_das_formas(&bytes, conversao.as_ref().ok());
        match conversao {
            Ok(conversao) if !conversao.macros.is_empty() || !conversao.eventos.is_empty() || !conversao.formularios.is_empty() => {
                // As macros entram sozinhas no editor (Alt+F11) desta pasta,
                // sem rodar. Só abrir o arquivo não o deixa "alterado".
                let alterado = self.alterado;
                self.instalar_vba(&arquivo, &conversao, false);
                self.alterado = alterado;
                self.status = trf!("Aberto: {} · VBA com {}, já no editor de macros (Alt+F11), em JavaScript. Nada roda sozinho.",
                    path.display(), resumo(&conversao));
                self.vba_pendente = Some(VbaPendente { arquivo, conversao });
            }
            Ok(_) => {}
            // .xlsm sem vbaProject.bin: só os dados.
            Err(e) => feathersheet_core::diario::anotar(trf!("[xlsm] sem macros: {e}")),
        }
    }

    /// Macros → Importar VBA: a do `.xlsm` aberto, ou escolher um arquivo.
    pub(super) fn importar_vba(&mut self) -> Task<Msg> {
        if let Some(p) = self.vba_pendente.take() {
            self.instalar_vba(&p.arquivo, &p.conversao, true);
            return Task::none();
        }
        Task::perform(async {
            let Some(f) = rfd::AsyncFileDialog::new()
                .add_filter(tr!("Pasta do Excel com macros"), &["xlsm", "xlam"])
                .pick_file().await else { return Ok(None) };
            let nome = std::path::Path::new(f.file_name().as_str()).file_stem()
                .and_then(|s| s.to_str()).unwrap_or("planilha").to_string();
            Ok(Some((nome, f.read().await)))
        }, Msg::VbaLido)
    }

    pub(super) fn vba_lido(&mut self, lido: Result<Option<(String, Vec<u8>)>, String>) {
        match lido {
            Ok(Some((arquivo, bytes))) => match feathersheet_vba::converter_xlsm(&bytes, &arquivo) {
                Ok(c) => self.instalar_vba(&arquivo, &c, true),
                Err(e) => self.status = e,
            },
            Ok(None) => {}
            Err(e) => self.status = e,
        }
    }

    /// Os formulários convertidos entram na pasta, no lugar dos de mesmo nome. `novos` são os
    /// módulos na ordem de `c.macros` (com o nome que cada um recebeu na biblioteca).
    fn instalar_formularios(&mut self, c: &Conversao, novos: &[feather_macros::Macro]) {
        use feathersheet_macros::formularios;
        if c.formularios.is_empty() { return; }
        let mut lista = formularios::ler(self.workbook.formularios.as_deref()).unwrap_or_default();
        for f in &c.formularios {
            let modulo = c.macros.iter().position(|m| m.nome.eq_ignore_ascii_case(&f.desenho.nome) && m.modulo.eq_ignore_ascii_case(&f.desenho.nome))
                .and_then(|i| novos.get(i)).map(|m| m.name.as_str());
            let form = formulario_da_conversao(f, modulo);
            lista.retain(|x| !x.nome.eq_ignore_ascii_case(&form.nome));
            lista.push(form);
        }
        lista.truncate(formularios::MAX_FORMULARIOS);
        match formularios::gravar(&lista) {
            Ok(json) => { self.workbook.formularios = json; self.alterado = true; }
            Err(e) => self.status = e,
        }
    }

    fn instalar_vba(&mut self, arquivo: &str, c: &Conversao, abrir_no_editor: bool) {
        // Reabrir o mesmo arquivo substitui os módulos dele em vez de
        // acrescentar cópias "_2" (cada módulo diz na 1ª linha de onde veio).
        let mut base = self.macro_catalogo.clone();
        base.macros.retain(|m| !veio_do_arquivo(m, arquivo));
        let novos = modulos_da_conversao(&base, arquivo, c);
        self.instalar_formularios(c, &novos);
        if novos.is_empty() && !c.formularios.is_empty() {
            self.status = trf!("{} formulário(s) do VBA de {arquivo} importado(s): Macros → Formulários (Alt+F11).", c.formularios.len());
            return;
        }
        if novos.is_empty() {
            self.status = tr!("O VBA deste arquivo não tem macros para importar.").into();
            return;
        }
        // Os botões das formas chamam a macro pelo nome base; se a biblioteca
        // já tinha esse nome, a importada ganhou "_2": os botões acompanham.
        let trocas: Vec<(String, String)> = c.macros.iter().zip(&novos)
            .map(|(m, novo)| (nome_modulo(&m.nome), novo.name.clone()))
            .filter(|(base, novo)| base != novo).collect();
        if !trocas.is_empty() {
            for i in 0..self.workbook.len() {
                let Some(aba) = self.workbook.aba_mut(i) else { continue };
                let Some(bt) = aba.sheet.bt.as_deref() else { continue };
                let Ok(mut itens) = serde_json::from_str::<Vec<serde_json::Value>>(bt) else { continue };
                for item in &mut itens {
                    let atual = item["macroName"].as_str().unwrap_or("").to_string();
                    if let Some((_, novo)) = trocas.iter().find(|(base, _)| *base == atual) {
                        item["macroName"] = serde_json::json!(novo);
                    }
                }
                aba.sheet.bt = serde_json::to_string(&itens).ok();
            }
        }
        // Os eventos de aba rodam sozinhos: guarda qual módulo é de qual aba.
        self.eventos_vba = c.eventos.iter().enumerate().filter_map(|(i, e)| Some(eventos_vba::EventoVba {
            aba: e.aba.clone()?, evento: e.evento.to_string(),
            modulo: novos.get(c.macros.len() + i)?.name.clone(),
        })).collect();
        let primeiro = novos[0].clone();
        let mut catalogo = base;
        let n = novos.len();
        catalogo.macros.extend(novos);
        if self.persistir_modulos(catalogo, &trf!("VBA de {arquivo} importado ({})", resumo(c))) {
            if !abrir_no_editor { return; }
            self.macro_nome = primeiro.name.clone();
            self.macro_codigo = pelicano::Content::with_text(&primeiro.code);
            self.macro_visual.documento = format!("modulo:{}", primeiro.name);
            self.status = trf!("{n} módulo(s) do VBA de {arquivo} no editor de macros (Alt+F11). Nada roda sozinho.");
        } else {
            self.status = self.macro_saida.clone();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_vba::converter_texto;

    #[test]
    fn reabrir_reconhece_os_modulos_do_mesmo_arquivo() {
        let c = converter_texto(&[
            ("Modulo1", "Sub Ola()\n  Range(\"A1\").Value = 1\nEnd Sub\n"),
            ("EstaPasta_de_trabalho", "Private Sub Workbook_Open()\n  MsgBox \"oi\"\nEnd Sub\n"),
        ], &[]);
        let mods = modulos_da_conversao(&Default::default(), "Vendas", &c);
        assert!(mods.iter().all(|m| veio_do_arquivo(m, "Vendas")), "{:?}", mods.iter().map(|m| m.code.lines().next()).collect::<Vec<_>>());
        assert!(mods.iter().all(|m| !veio_do_arquivo(m, "Vend")));
        assert!(!veio_do_arquivo(&feather_macros::Macro { name: "x".into(), code: "log(1)".into(), auto: false }, "Vendas"));
    }

    #[test]
    fn macros_eventos_e_pendencias_viram_modulos_sem_colidir() {
        let c = converter_texto(&[
            ("Modulo1", "Sub Ola()\n  Range(\"A1\").Value = 1\nEnd Sub\n"),
            ("EstaPasta_de_trabalho", "Private Sub Workbook_Open()\n  MsgBox \"oi\"\nEnd Sub\n"),
        ], &[]);
        let mut catalogo = feather_macros::Catalogo::default();
        catalogo.macros.push(feather_macros::Macro { name: "OLA".into(), code: String::new(), auto: false });
        let mods = modulos_da_conversao(&catalogo, "Vendas 2026", &c);
        assert!(mods.iter().any(|m| m.name == "Ola_2"), "{:?}", mods.iter().map(|m| &m.name).collect::<Vec<_>>());
        assert!(mods.iter().all(|m| !m.auto), "nada roda sozinho");
        assert!(mods.iter().any(|m| m.name.starts_with("Evento_") && m.code.contains("execute à mão")), "Workbook_Open continua à mão");
        assert!(mods.iter().all(|m| m.name.chars().all(|ch| ch.is_alphanumeric() || ch == '_')));
        // O conjunto passa na validação da biblioteca (nomes únicos, tamanho).
        catalogo.macros.extend(mods);
        let json = catalogo.exportar_json().unwrap();
        assert!(feather_macros::Catalogo::importar_json(&json).is_ok());
    }

    /// Com os .xlsm reais do usuário (só leitura): `cargo test xlsm_reais -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn xlsm_reais_abrem_e_o_vba_converte() {
        let casa = std::env::var("HOME").unwrap_or_default();
        for nome in ["Downloads/Fluxo de caixa.xlsm", "Downloads/precioso.xlsm",
            "Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm"] {
            let path = std::path::PathBuf::from(&casa).join(nome);
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let t = std::time::Instant::now();
            let mut app = App::default();
            app.carregar(path.clone(), bytes).expect("abre como xlsx");
            let abas = app.workbook.len();
            let celulas: usize = (0..abas).map(|i| app.workbook.aba(i).unwrap().sheet.used_rect().r1 as usize).sum();
            let vba = app.vba_pendente.as_ref().map(|p| resumo(&p.conversao));
            println!("{nome}: {abas} abas, {celulas} linhas usadas, VBA: {vba:?}, {:.2} s · {}", t.elapsed().as_secs_f64(), app.status);
            if let Some(p) = &app.vba_pendente {
                let mods = modulos_da_conversao(&feather_macros::Catalogo::default(), &p.arquivo, &p.conversao);
                println!("   módulos: {:?}", mods.iter().map(|m| m.name.as_str()).take(12).collect::<Vec<_>>());
            }
        }
    }

    /// Ordenar (`Sort.SortFields`), validação (`Validation.Add`), rolar a janela e
    /// excluir células deslocando: do VBA ao resultado na planilha.
    #[test]
    fn vba_de_ordenar_validar_e_deslocar_roda() {
        use feathersheet_core::planilha::workbook::Workbook;
        let vba = concat!(
            "Sub Arrumar()\n",
            "  ActiveWindow.SmallScroll Down:=-12\n",
            "  Sheet1.Range(\"A1:B4\").Select\n",
            "  ActiveWorkbook.Worksheets(\"Dados\").Sort.SortFields.Clear\n",
            "  ActiveWorkbook.Worksheets(\"Dados\").Sort.SortFields.Add Key:=Range(Cells(1, \"A\"), Cells(4, \"A\")), SortOn:=xlSortOnValues, Order:=xlAscending, DataOption:=xlSortNormal\n",
            "  With ActiveWorkbook.Worksheets(\"Dados\").Sort\n",
            "    .SetRange Range(Cells(1, \"A\"), Cells(4, \"B\"))\n",
            "    .Header = xlGuess\n",
            "    .MatchCase = False\n",
            "    .Orientation = xlTopToBottom\n",
            "    .SortMethod = xlPinYin\n",
            "    .Apply\n",
            "  End With\n",
            "  Range(\"D1\").Select\n",
            "  With Selection.Validation\n",
            "    .Delete\n",
            "    .Add Type:=xlValidateList, AlertStyle:=xlValidAlertStop, Operator:= _\n",
            "    xlBetween, Formula1:=\"75,112.5,225,outro:\"\n",
            "    .IgnoreBlank = True\n",
            "  End With\n",
            "  Range(\"E1\").Select\n",
            "  With Selection.Validation\n",
            "    .Delete\n",
            "    .Add Type:=xlValidateInputOnly, AlertStyle:=xlValidAlertStop, Operator _\n",
            "    :=xlBetween\n",
            "  End With\n",
            "  Range(\"B2\").Select\n",
            "  Selection.Delete Shift:=xlUp\n",
            "End Sub\n");
        let c = feathersheet_vba::converter_texto(&[("Módulo1", vba)], &[("Sheet1", "Dados")]);
        let m = c.macros.iter().find(|m| m.nome == "Arrumar").expect("macro");
        assert!(!m.codigo.contains("// VBA não convertido ("), "{}", m.codigo);
        let mut wb = Workbook::new();
        assert!(wb.renomear_aba(0, "Dados"));
        for (r, (a, b)) in [("Growatt", "1"), ("ABB", "2"), ("Sungrow", "3"), ("Fronius", "4")].iter().enumerate() {
            wb.escrever_celula(0, r as u32, 0, (*a).into());
            wb.escrever_celula(0, r as u32, 1, (*b).into());
        }
        let antes = feathersheet_macros::executar_com(&mut wb, 0, "sheet.setDataValidation('E1', {type: 'list', values: ['x']});", Default::default());
        assert!(antes.erro.is_none() && crate::grade::lista_suspensa::opcoes_validacao(&wb, 0, 0, 4).is_some());
        let r = feathersheet_macros::executar_com(&mut wb, 0, &m.codigo, Default::default());
        assert!(r.erro.is_none(), "{:?}\n{}", r.erro, m.codigo);
        let s = &wb.aba(0).unwrap().sheet;
        let col = |c| (0..4).map(|l| s.get_raw(l, c)).collect::<Vec<_>>();
        assert_eq!(col(0), ["ABB", "Fronius", "Growatt", "Sungrow"], "sem cabeçalho: a linha 1 também ordena");
        // B2 saiu e o resto da coluna B subiu: 2,4,1,3 → 2,1,3,"".
        assert_eq!(col(1), ["2", "1", "3", ""]);
        assert_eq!(crate::grade::lista_suspensa::opcoes_validacao(&wb, 0, 0, 3).unwrap(), ["75", "112.5", "225", "outro:"]);
        assert!(crate::grade::lista_suspensa::opcoes_validacao(&wb, 0, 0, 4).is_none(), "xlValidateInputOnly tira a regra");
    }

    /// Executa cada macro convertida do .xlsm real numa cópia em memória
    /// da própria planilha: `cargo test macros_reais_rodam -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn macros_reais_rodam() {
        let casa = std::env::var("HOME").unwrap_or_default();
        for nome in ["Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm",
            "Downloads/precioso.xlsm", "Downloads/Fluxo de caixa.xlsm"] {
            let path = std::path::PathBuf::from(&casa).join(nome);
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let mut base = App::default();
            base.carregar(path.clone(), bytes).expect("abre");
            let Some(p) = base.vba_pendente.clone() else { continue };
            let (mut ok, mut erro) = (0, 0);
            println!("== {nome}");
            for m in &p.conversao.macros {
                let mut app = App::default();
                app.carregar(path.clone(), std::fs::read(&path).unwrap()).unwrap();
                let t = std::time::Instant::now();
                // Responde Sim a toda pergunta, para a macro andar até o fim.
                let ctx = feathersheet_macros::Contexto { arquivo: Some(path.clone()),
                    selecao: None, respostas: vec![true; 50], valores: None };
                let r = feathersheet_macros::executar_com(&mut app.workbook, app.folha, &m.codigo, ctx);
                let ms = t.elapsed().as_millis();
                match &r.erro {
                    None => { ok += 1; println!("   ok    {:<28} {ms} ms · alterou: {}", m.nome, r.alterou); }
                    Some(e) => { erro += 1; println!("   ERRO  {:<28} {ms} ms · {}", m.nome, e.lines().next().unwrap_or(""));
                        if std::env::var("MOSTRAR_CODIGO").is_ok() { println!("{}", m.codigo); } }
                }
            }
            println!("   {ok} rodaram, {erro} com erro");
        }
    }

    /// O Worksheet_Change real da MAIN da v6.0a, disparado pela edição:
    /// `cargo test evento_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn evento_real_da_main_roda_ao_mudar() {
        let casa = std::env::var("HOME").unwrap_or_default();
        let path = std::path::PathBuf::from(casa).join("Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm");
        let Ok(bytes) = std::fs::read(&path) else { return };
        let mut app = App::default();
        app.carregar(path, bytes).expect("abre");
        let c = app.vba_pendente.clone().unwrap().conversao;
        // Instala só na memória (sem gravar a biblioteca do disco).
        app.macro_catalogo = Default::default();
        let mods = modulos_da_conversao(&app.macro_catalogo, "v6", &c);
        app.eventos_vba = c.eventos.iter().enumerate().filter_map(|(i, e)| Some(eventos_vba::EventoVba {
            aba: e.aba.clone()?, evento: e.evento.to_string(), modulo: mods.get(c.macros.len() + i)?.name.clone() })).collect();
        app.macro_catalogo.macros = mods;
        println!("eventos: {:?}", app.eventos_vba);
        let main = app.workbook.indice_por_nome("MAIN").unwrap();
        app.update(Msg::Aba(main));
        app.update(Msg::Selecionar(18, 1));
        app.update(Msg::Editar("NÃO".into()));
        let t = std::time::Instant::now();
        app.update(Msg::Confirmar);
        println!("Confirmar + evento: {} ms", t.elapsed().as_millis());
        println!("status: {} · saída: {}", app.status, app.macro_saida);
        println!("C1 = {:?} · C2 = {:?}", app.workbook.aba(main).unwrap().sheet.get_raw(0, 2), app.workbook.aba(main).unwrap().sheet.get_raw(1, 2));
    }

    /// As listas de validação dos .xlsx/.xlsm reais (só leitura):
    /// `cargo test listas_reais -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn listas_reais_de_validacao() {
        use feathersheet_core::operacoes::validation::Validacao;
        let casa = std::env::var("HOME").unwrap_or_default();
        for nome in ["Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm",
            "Downloads/QUF+Participação+Social_versão+retificada+em+18_set_26.xlsx",
            "Downloads/MEMORIA_DE_CALCULO_-_PREVISAO_DE_CUSTOS_ORCAMENTARIOS.xlsx",
            "Downloads/Rio_Doce_Explosao_Orcamentaria.xlsx"] {
            let path = std::path::PathBuf::from(&casa).join(nome);
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let mut app = App::default();
            app.carregar(path, bytes).expect("abre");
            println!("== {nome} · {} nomes: {:?}", app.workbook.nomeados().len(), app.workbook.nomeados().iter().take(3).map(|n| (&n.nome, &n.area)).collect::<Vec<_>>());
            for (aba, cel) in [("MAIN", "E28"), ("MAIN", "B28"), ("2. Eixo - Ações", "C6"), ("2. Eixo - Ações", "C7")] {
                if let (Some(i), Some((r, c))) = (app.workbook.indice_por_nome(aba), feathersheet_core::celulas::refs::parse_cell_ref(cel)) {
                    println!("   valor {aba}!{cel} = {:?}", app.workbook.valor(i, r, c));
                }
            }
            if let Some(i) = app.workbook.indice_por_nome("MAIN") {
                app.workbook.escrever_celula(i, 27, 4, "Growatt".into());
                let r = feathersheet_core::operacoes::validation::Regra::Formula { formula: "=INDIRECT(E28)".into() };
                println!("   E28=Growatt → E29: {:?}", app.workbook.opcoes_da_lista(i, 28, 4, &r));
            }
            for i in 0..app.workbook.len() {
                let aba = app.workbook.aba(i).unwrap();
                let Some(va) = aba.sheet.va.as_ref() else { continue };
                let regras: Vec<Validacao> = serde_json::from_str(va).unwrap();
                for v in &regras {
                    let r = &v.ranges[0];
                    let opcoes = app.workbook.opcoes_da_lista(i, r.r0, r.c0, &v.regra);
                    let mostra = opcoes.map(|o| format!("{} opções: {:?} · {:?}", o.len(), o.iter().take(4).collect::<Vec<_>>(), if o.is_empty() { Some(&v.regra) } else { None }))
                        .unwrap_or_else(|| format!("{:?}", v.regra));
                    println!("   {} {}{}: {mostra}", aba.sheet.name, feathersheet_core::celulas::refs::col_name(r.c0), r.r0 + 1);
                }
            }
        }
    }

    /// #67: o formulário do VBA entra na pasta, e o botão chama a função do módulo dele.
    #[test]
    fn formulario_do_vba_entra_com_a_macro_do_botao() {
        use feathersheet_vba::formulario::{ControleVba, FormularioVba, TipoVba};
        use feathersheet_vba::{ModuloVba, ProjetoVba, TipoDeModulo};
        let ctl = |nome: &str, tipo| ControleVba { nome: nome.into(), tipo, largura: 80.0, altura: 20.0, ..Default::default() };
        let projeto = ProjetoVba {
            modulos: vec![
                ModuloVba { nome: "Modulo1".into(), tipo: TipoDeModulo::Padrao, codigo: "Sub Abrir()\n  UserForm1.Show\nEnd Sub\n".into() },
                ModuloVba { nome: "UserForm1".into(), tipo: TipoDeModulo::Classe, codigo:
                    "Private Sub UserForm_Initialize()\n  ComboBox1.AddItem \"Sul\"\nEnd Sub\nPrivate Sub CommandButton1_Click()\n  Range(\"A1\").Value = TextBox1.Value\n  Unload Me\nEnd Sub\n".into() },
            ],
            formularios: vec![FormularioVba { nome: "UserForm1".into(), titulo: "Cadastro".into(), largura: 300.0, altura: 200.0,
                controles: vec![ctl("TextBox1", TipoVba::Texto), ctl("ComboBox1", TipoVba::Lista), ControleVba { padrao: true, ..ctl("CommandButton1", TipoVba::Botao) }],
                ..Default::default() }],
            ..Default::default()
        };
        let c = feathersheet_vba::converter(&projeto, "Cadastro");
        let mut app = App::default();
        app.instalar_vba("Cadastro", &c, false);
        let lista = feathersheet_macros::formularios::ler(app.workbook.formularios.as_deref()).unwrap();
        let f = feathersheet_macros::formularios::obter(&lista, "UserForm1").expect("formulário");
        assert_eq!(f.titulo, "Cadastro");
        assert_eq!(f.controles[1].opcoes, vec!["Sul"]);
        assert!(f.controles[1].livre);
        let botao = &f.controles[2];
        assert!(botao.padrao);
        let macro_nome = botao.macro_nome.clone().expect("o botão roda a macro");
        let (_, codigo) = feathersheet_macros::procedimentos::codigo_da_macro(&app.macro_catalogo, &macro_nome).expect("a macro existe na biblioteca");
        assert!(codigo.contains("sheet.hideComponent();"), "{codigo}");
        let abrir = feathersheet_macros::procedimentos::codigo_da_macro(&app.macro_catalogo, "Abrir").unwrap().1;
        assert!(abrir.contains("sheet.showComponent(\"UserForm1\")"), "{abrir}");
    }
}
