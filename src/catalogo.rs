//! Catálogo (spec da fase 6, §3.4 e §4.1): uma ficha por coisa que aparece no relatório e a
//! conferência de cada fonte (§4.6). Dados compilados; `anexar` só põe `card` e a linha do
//! Ajustes, nunca muda número.

use crate::anon::DOBRADO;
use crate::conselho::casa;
use crate::linhas;
use crate::sources::categorias;
use serde_json::{Map, Value, json};

#[derive(Clone, Copy)]
pub struct Txt {
    pub pt: &'static str,
    pub en: &'static str,
}

pub enum Casa {
    Caminho(&'static str),
    Processo(&'static str),
    Servico(&'static str),
    Volume(&'static str),
    Termo(&'static str),
}

pub enum Sozinho {
    Sim(Txt),
    Quando(Txt),
    Nao,
    NaoInformado,
}

pub enum Painel {
    Armazenamento,
    TimeMachine,
    Spotlight,
}

pub enum Acao {
    Finder,
    Ajustes(Painel),
    AppDono,
    Comando(&'static str),
    Reiniciar,
    EncerrarSessao,
    Nada,
}

pub enum Ref {
    Interno(&'static str),
    Apple(&'static str),
    Relato(&'static str),
    Medido(&'static str),
}

pub struct Ficha {
    pub id: &'static str,
    pub casa: &'static [Casa],
    pub o_que_e: Txt,
    pub dono: Txt,
    pub sozinho: Sozinho,
    pub nivel: &'static str,
    pub perde: Txt,
    pub passo: Txt,
    pub agir: &'static [Acao],
    pub fonte: &'static [Ref],
    pub conferido: &'static str,
}

pub struct Conf {
    pub checked: &'static str,
    pub on: &'static [&'static str],
    pub at: &'static str,
    pub apple_command: Option<&'static str>,
}

const fn t(pt: &'static str, en: &'static str) -> Txt {
    Txt { pt, en }
}

use Casa::{Caminho, Processo, Servico, Termo, Volume};

/// Prefixo das pastas de recursos baixados pelo macOS.
macro_rules! asset {
    ($x:literal) => {
        Caminho(concat!("/System/Library/AssetsV2/com_apple_MobileAsset_", $x))
    };
}

const MACOS: Txt = t("macOS", "macOS");
const VOCE: Txt = t("Você.", "You.");
const NADA: Txt = t("Nada a fazer.", "Nothing to do.");
const NAO_MEXER: Txt = t("Não mexer.", "Do not touch.");
const FALTA: Sozinho = Sozinho::Sim(t("quando falta espaço", "when space runs low"));
const VOLTA: Txt = t("Nada que não volte: o macOS recria ou baixa de novo quando precisa.", "Nothing that does not come back: macOS recreates or downloads it again when needed.");
const MEDICAO: Txt = t("Nada se perde; a medição fica mais completa.", "Nothing is lost; the measurement gets more complete.");
const BAIXA: Txt = t("Tempo e banda para baixar de novo.", "Time and bandwidth to download it again.");
const GERENCIADO: Txt = t("Gerenciado pelo macOS.", "Managed by macOS.");
const F5: &[Ref] = &[Ref::Interno("docs/spec-fase5.md §6")];
const SEGURO: &[Ref] = &[Ref::Interno("docs/04-catalogo-de-alvos.md Seguro")];
const CUSTO: &[Ref] = &[Ref::Interno("docs/04-catalogo-de-alvos.md Custo")];
const CONDICIONAL: &[Ref] = &[Ref::Interno("docs/04-catalogo-de-alvos.md Condicional")];
const PROIBIDO: &[Ref] = &[Ref::Interno("docs/04-catalogo-de-alvos.md Proibido")];
const MAPA1: &[Ref] = &[Ref::Medido("research/mapa-de-fontes.md §1")];
const MAPA3: &[Ref] = &[Ref::Medido("research/mapa-de-fontes.md §3")];
const F6: &[Ref] = &[Ref::Interno("docs/spec-fase6.md §6.2")];
const AO_REINICIAR: Txt = t("O trabalho aberto que não foi salvo.", "Open work that was not saved.");

/// Base das fichas do que o macOS gerencia; cada ficha troca o que for dela.
const B: Ficha = Ficha {
    id: "",
    casa: &[],
    o_que_e: t("", ""),
    dono: MACOS,
    sozinho: Sozinho::NaoInformado,
    nivel: "system_handles",
    perde: VOLTA,
    passo: NADA,
    agir: &[Acao::Nada],
    fonte: MAPA1,
    conferido: "27.0",
};

/// A ordem é a prioridade do casamento: Cinebench antes de `preferences_anomaly`; recursos do
/// macOS (`asset_*`, Apple Intelligence) antes de `system_folders`.
pub const CATALOGO: &[Ficha] = &[
    // ---- regras no código (conselho::conselhos) ----
    Ficha {
        id: "grant_full_disk_access",
        o_que_e: t("Permissão do macOS que deixa o sdtally ler pastas protegidas, como Mail, Mensagens e Safari.", "macOS permission that lets sdtally read protected folders such as Mail, Messages and Safari."),
        dono: t("Você, em Ajustes > Privacidade e Segurança.", "You, in System Settings > Privacy & Security."),
        sozinho: Sozinho::Nao,
        nivel: "measurement",
        perde: MEDICAO,
        passo: t("Sem o Acesso Total ao Disco, Mail, Mensagens, Safari e outras pastas protegidas ficam bloqueadas pelo macOS. Ative em Permissões (⌘,) e meça de novo.", "Grant Full Disk Access to sdtally (or the app) and measure again."),
        fonte: &[Ref::Interno("docs/03-restricoes-de-plataforma.md")],
        ..B
    },
    Ficha {
        id: "measure_as_admin",
        o_que_e: t("Medição com a senha de administrador ou o Root sem Senha, que abre as pastas do sistema.", "Measuring with the administrator password or Passwordless Root, which opens the system folders."),
        dono: VOCE,
        sozinho: Sozinho::Nao,
        nivel: "measurement",
        perde: MEDICAO,
        passo: t("As pastas do sistema só abrem com a senha de administrador ou o Root sem Senha; sem elas, o que não abriu fica bloqueado pelo macOS.", "Measure as administrator; what did not open stays blocked by macOS."),
        fonte: &[Ref::Interno("docs/03-restricoes-de-plataforma.md")],
        ..B
    },
    Ficha {
        id: "time_machine_snapshots",
        o_que_e: t("Snapshots locais que o Time Machine cria no volume Data entre os backups.", "Local snapshots Time Machine creates on the Data volume between backups."),
        dono: t("macOS (Time Machine)", "macOS (Time Machine)"),
        sozinho: FALTA,
        perde: t("O ponto de restauração local daquele horário.", "The local restore point for that time."),
        passo: t("Nada a fazer: o macOS apaga esses snapshots sozinho quando falta espaço. Para ver a lista, use o comando abaixo no Terminal.", "Nothing to do: macOS deletes local Time Machine snapshots when space runs low. To list them: tmutil listlocalsnapshots /"),
        agir: &[Acao::Comando("tmutil listlocalsnapshots /")],
        fonte: MAPA3,
        ..B
    },
    Ficha {
        id: "other_snapshots",
        o_que_e: t("Snapshot criado por outro app, como um de backup ou de clonagem.", "Snapshot created by another app, such as a backup or cloning app."),
        dono: t("O app que o criou.", "The app that created it."),
        nivel: "check_first",
        perde: t("O ponto de restauração que esse app guardou.", "The restore point that app kept."),
        passo: t("Snapshot criado por outro app, como um de backup ou de clonagem. Gerencie pelo app que o criou.", "Snapshot from another app (backup or cloning); manage it in the app that created it."),
        agir: &[Acao::AppDono],
        fonte: MAPA3,
        ..B
    },
    Ficha {
        id: "purgeable_auto",
        o_que_e: t("Espaço que o macOS pode liberar sozinho: caches e arquivos marcados como removíveis.", "Space macOS can free on its own: caches and files marked as removable."),
        sozinho: FALTA,
        passo: t("Nada a fazer: o macOS libera esse espaço quando precisar.", "Nothing to do: macOS frees purgeable space when it needs it."),
        ..B
    },
    Ficha {
        id: "deleted_open",
        casa: &[Termo("deleted_open")],
        o_que_e: t("Arquivos já apagados que um app ainda mantém abertos; o espaço só volta quando ele os solta.", "Deleted files an app still keeps open; the space only returns when it lets go of them."),
        dono: t("O app que segura os arquivos.", "The app holding the files."),
        sozinho: Sozinho::Quando(t("quando o app solta os arquivos ou ao reiniciar", "when the app lets go of the files or on restart")),
        nivel: "safe",
        perde: t("O trabalho não salvo do app, se ele fechar sem salvar.", "The app's unsaved work, if it quits without saving."),
        passo: t("Reiniciar o Mac libera esse espaço.", "Restarting the Mac frees this space."),
        agir: &[Acao::AppDono, Acao::Reiniciar],
        fonte: MAPA3,
        ..B
    },
    Ficha {
        id: "swap_large",
        o_que_e: t("Memória que foi parar no disco (swap).", "Memory that ended up on disk (swap)."),
        sozinho: Sozinho::Quando(t("ao fechar apps pesados ou reiniciar", "when heavy apps quit or on restart")),
        perde: t("Nada; ao reiniciar, o trabalho aberto que não foi salvo.", "Nothing; on restart, open work that was not saved."),
        passo: t("O Mac está usando o disco como memória. Fechar apps pesados ou reiniciar devolve o espaço.", "The Mac is using the disk as memory; quitting heavy apps or restarting gives it back."),
        agir: &[Acao::Reiniciar],
        fonte: &[Ref::Interno("docs/spec-fase5.md §6")],
        ..B
    },
    Ficha {
        id: "apple_intelligence_models",
        casa: &[asset!("UAF_FM_GenerativeModels"), asset!("UAF_FM_Visual")],
        o_que_e: t("Modelos do Apple Intelligence, baixados pelo macOS.", "Apple Intelligence models, downloaded by macOS."),
        perde: t("Os recursos do Apple Intelligence até baixarem de novo.", "Apple Intelligence features until they download again."),
        passo: t("Ficam dentro de Dados do Sistema e o macOS gerencia o tamanho. Não apague a pasta.", "Apple Intelligence models; macOS manages them. Do not delete the folder."),
        ..B
    },
    Ficha {
        id: "restart_worth",
        o_que_e: t("Reiniciar o Mac quando o swap e os arquivos apagados que apps ainda seguram somam bastante.", "Restarting the Mac when swap and deleted files apps still hold add up to a lot."),
        dono: VOCE,
        sozinho: Sozinho::Nao,
        nivel: "safe",
        perde: AO_REINICIAR,
        passo: t("Reiniciar devolve o swap (sai da linha macOS do Ajustes) e os arquivos apagados que apps ainda seguram (sai de Dados do Sistema). Salve o trabalho aberto antes. O swap volta com o uso.", "Restarting gives back swap (it leaves the macOS line in Settings) and deleted files apps still hold (it leaves System Data). Save open work first. Swap comes back with use."),
        agir: &[Acao::Reiniciar],
        fonte: &[Ref::Interno("docs/spec-fase6.md §4.2"), Ref::Medido("research/mapa-de-fontes.md §3")],
        ..B
    },
    Ficha {
        id: "tm_backup_stuck",
        o_que_e: t("O Time Machine tentou fazer backup várias vezes sem registrar um novo; enquanto isso, o snapshot de base fica no disco.", "Time Machine tried to back up several times without recording a new backup; meanwhile the base snapshot stays on disk."),
        dono: t("macOS (Time Machine)", "macOS (Time Machine)"),
        nivel: "check_first",
        perde: t("Nada: é só conferir o destino do backup.", "Nothing: it is only a matter of checking the backup destination."),
        passo: t("O Time Machine tentou de novo várias vezes desde o último backup registrado, sem registrar outro, e o snapshot de base segue no disco. Veja o destino em Ajustes > Time Machine.", "Time Machine has tried again several times since the last recorded backup without recording another, and the base snapshot stays on disk. See the destination in System Settings > Time Machine."),
        agir: &[Acao::Ajustes(Painel::TimeMachine), Acao::Comando("tmutil listlocalsnapshots /")],
        fonte: &[Ref::Medido("research/mapa-de-fontes.md §3"), Ref::Interno("docs/spec-fase6.md §4.4")],
        ..B
    },
    Ficha {
        id: "chronod_datavault",
        casa: &[Processo("chronod")],
        o_que_e: t("Cópias apagadas do banco do LaunchServices que o chronod (widgets) ainda mantém abertas.", "Deleted copies of the LaunchServices database that chronod (widgets) still keeps open."),
        dono: t("macOS (chronod, dos widgets)", "macOS (chronod, for widgets)"),
        sozinho: Sozinho::Quando(t("ao encerrar a sessão ou reiniciar", "on logging out or restarting")),
        nivel: "safe",
        perde: AO_REINICIAR,
        passo: t("Encerrar a sessão ou reiniciar devolve o espaço; salve o trabalho aberto antes. O tamanho é o lógico do lsof e fica fora da soma.", "Logging out or restarting gives the space back; save open work first. The size is the lsof logical size and stays out of the sum."),
        agir: &[Acao::EncerrarSessao, Acao::Reiniciar],
        fonte: MAPA3,
        ..B
    },
    // ---- conselho::REGRAS, na mesma ordem ----
    Ficha {
        id: "trash",
        casa: &[Caminho("~/.Trash")],
        o_que_e: t("Arquivos que você mandou para o Lixo.", "Files you moved to the Trash."),
        dono: VOCE,
        sozinho: Sozinho::Quando(t("se a opção de remover itens do Lixo após 30 dias estiver ligada no Finder", "if the Finder option to remove items from the Trash after 30 days is on")),
        nivel: "safe",
        perde: t("Os arquivos do Lixo, de vez.", "The files in the Trash, for good."),
        passo: t("Esvazie o Lixo no Finder. O espaço volta quando o Lixo é esvaziado.", "Empty the Trash."),
        agir: &[Acao::Finder],
        fonte: F5,
        ..B
    },
    Ficha {
        id: "logs",
        casa: &[Caminho("~/Library/Logs")],
        o_que_e: t("Logs dos apps.", "App logs."),
        dono: t("Os apps que os gravam.", "The apps that write them."),
        nivel: "safe",
        perde: t("O histórico de erros dos apps.", "The apps' error history."),
        passo: t("Mova para o Lixo pelo Finder; os apps criam de novo. O espaço volta quando o Lixo é esvaziado.", "Move to the Trash in Finder; apps recreate them. Space returns when the Trash is emptied."),
        agir: &[Acao::Finder],
        fonte: SEGURO,
        ..B
    },
    Ficha {
        id: "homebrew_cache",
        casa: &[Caminho("~/Library/Caches/Homebrew")],
        o_que_e: t("Downloads guardados pelo Homebrew.", "Downloads kept by Homebrew."),
        dono: t("Homebrew", "Homebrew"),
        nivel: "safe",
        perde: t("Os downloads, que o Homebrew baixa de novo se precisar; sem --dry-run, também as versões antigas das fórmulas.", "The downloads, which Homebrew fetches again if needed; without --dry-run, also old formula versions."),
        passo: t("Rode brew cleanup --dry-run para ver o que sairia. Sem o --dry-run, ele também remove versões antigas das fórmulas instaladas.", "Run brew cleanup --dry-run to see what it would remove. Without --dry-run it also removes old versions of installed formulae."),
        agir: &[Acao::Comando("brew cleanup --dry-run")],
        fonte: SEGURO,
        ..B
    },
    Ficha {
        id: "npm_cache",
        casa: &[Caminho("~/.npm/_cacache")],
        o_que_e: t("Cache de pacotes do npm.", "npm package cache."),
        dono: t("npm", "npm"),
        nivel: "safe",
        perde: t("Nada; o npm baixa os pacotes de novo.", "Nothing; npm downloads the packages again."),
        passo: t("Rode npm cache clean --force; o npm baixa os pacotes de novo quando precisar.", "Run npm cache clean --force; npm downloads packages again when needed."),
        agir: &[Acao::Comando("npm cache clean --force")],
        fonte: SEGURO,
        ..B
    },
    Ficha {
        id: "go_build_cache",
        casa: &[Caminho("~/Library/Caches/go-build")],
        o_que_e: t("Cache de compilação do Go.", "Go build cache."),
        dono: t("Go", "Go"),
        nivel: "safe",
        perde: t("Tempo: o próximo build recompila.", "Time: the next build recompiles."),
        passo: t("Rode go clean -cache; o próximo build recompila.", "Run go clean -cache; the next build recompiles."),
        agir: &[Acao::Comando("go clean -cache")],
        fonte: SEGURO,
        ..B
    },
    Ficha {
        id: "pip_cache",
        casa: &[Caminho("~/Library/Caches/pip")],
        o_que_e: t("Cache de pacotes do pip.", "pip package cache."),
        dono: t("pip", "pip"),
        nivel: "safe",
        perde: t("Nada; o pip baixa os pacotes de novo.", "Nothing; pip downloads the packages again."),
        passo: t("Rode pip cache purge; o pip baixa os pacotes de novo quando precisar.", "Run pip cache purge; pip downloads packages again when needed."),
        agir: &[Acao::Comando("pip cache purge")],
        fonte: SEGURO,
        ..B
    },
    Ficha {
        id: "updater_caches",
        casa: &[Caminho("~/Library/Caches/*.ShipIt"), Caminho("~/Library/Caches/*-updater"), Caminho("~/Library/Caches/node-gyp")],
        o_que_e: t("Instaladores de atualização de apps que já foram aplicados.", "App update installers that were already applied."),
        dono: t("Os apps que se atualizam sozinhos.", "The apps that update themselves."),
        nivel: "safe",
        perde: t("Nada; já foram aplicados.", "Nothing; they were already applied."),
        passo: t("Instaladores já aplicados: mova para o Lixo pelo Finder. O espaço volta quando o Lixo é esvaziado.", "Installers already applied: move to the Trash in Finder."),
        agir: &[Acao::Finder],
        fonte: SEGURO,
        ..B
    },
    Ficha {
        id: "xcode_derived_data",
        casa: &[Caminho("~/Library/Developer/Xcode/DerivedData")],
        o_que_e: t("Resultados de build e índices do Xcode.", "Xcode build products and indexes."),
        dono: t("Xcode", "Xcode"),
        sozinho: Sozinho::Nao,
        nivel: "cost",
        perde: t("Tempo: o próximo build recompila tudo.", "Time: the next build recompiles everything."),
        passo: t("Mova para o Lixo pelo Finder; o próximo build recompila tudo. O espaço volta quando o Lixo é esvaziado.", "Delete in Finder; the next build recompiles everything."),
        agir: &[Acao::Finder],
        fonte: CUSTO,
        ..B
    },
    Ficha {
        id: "simulators",
        casa: &[Caminho("~/Library/Developer/CoreSimulator/Devices")],
        o_que_e: t("Aparelhos de simulador do Xcode e os dados deles.", "Xcode simulator devices and their data."),
        dono: t("Xcode", "Xcode"),
        sozinho: Sozinho::Nao,
        nivel: "cost",
        perde: t("Os apps e dados dentro dos simuladores removidos.", "The apps and data inside the removed simulators."),
        passo: t("Rode xcrun simctl delete unavailable para tirar os que não servem mais; o resto, em Xcode > Devices and Simulators.", "Run xcrun simctl delete unavailable; manage the rest in Xcode > Devices and Simulators."),
        agir: &[Acao::Comando("xcrun simctl delete unavailable"), Acao::AppDono],
        fonte: CUSTO,
        ..B
    },
    Ficha {
        id: "simulator_runtimes",
        casa: &[Caminho("/Library/Developer/CoreSimulator")],
        o_que_e: t("Sistemas dos simuladores (iOS, watchOS e outros) instalados pelo Xcode.", "Simulator runtimes (iOS, watchOS and others) installed by Xcode."),
        dono: t("Xcode", "Xcode"),
        sozinho: Sozinho::Nao,
        nivel: "cost",
        perde: BAIXA,
        passo: t("Veja quais existem com xcrun simctl runtime list e remova os que não usa em Xcode > Settings > Components.", "See them with xcrun simctl runtime list; remove them in Xcode > Settings > Components."),
        agir: &[Acao::Comando("xcrun simctl runtime list"), Acao::AppDono],
        fonte: CUSTO,
        ..B
    },
    Ficha {
        id: "playwright_browsers",
        casa: &[Caminho("~/Library/Caches/ms-playwright")],
        o_que_e: t("Navegadores baixados pelo Playwright para testes.", "Browsers downloaded by Playwright for testing."),
        dono: t("Playwright", "Playwright"),
        sozinho: Sozinho::Nao,
        nivel: "cost",
        perde: BAIXA,
        passo: t("Só se você não usa o Playwright: mova para o Lixo pelo Finder. O espaço volta quando o Lixo é esvaziado.", "Only if you do not use Playwright: move to the Trash in Finder."),
        agir: &[Acao::Finder],
        fonte: CUSTO,
        ..B
    },
    Ficha {
        id: "ollama_models",
        casa: &[Caminho("~/.ollama/models")],
        o_que_e: t("Modelos de IA baixados pelo Ollama.", "AI models downloaded by Ollama."),
        dono: t("Ollama", "Ollama"),
        sozinho: Sozinho::Nao,
        nivel: "cost",
        perde: BAIXA,
        passo: t("Veja os modelos com ollama list e remova pelo próprio Ollama os que não usa.", "See them with ollama list and remove them through Ollama itself."),
        agir: &[Acao::Comando("ollama list"), Acao::AppDono],
        fonte: CUSTO,
        ..B
    },
    Ficha {
        id: "cinebench_assets",
        casa: &[Caminho("~/Library/Preferences/Maxon/Cinebench_*")],
        o_que_e: t("Cenas do benchmark Cinebench.", "Cinebench benchmark scenes."),
        dono: t("Cinebench", "Cinebench"),
        sozinho: Sozinho::Nao,
        nivel: "cost",
        perde: BAIXA,
        passo: t("Cenas do benchmark, que baixam de novo quando ele roda: mova para o Lixo pelo Finder. O espaço volta quando o Lixo é esvaziado.", "Benchmark assets; they download again when it runs."),
        agir: &[Acao::Finder],
        fonte: CUSTO,
        ..B
    },
    Ficha {
        id: "cloudkit_cache",
        casa: &[Caminho("~/Library/Caches/CloudKit")],
        o_que_e: t("Cache do iCloud (CloudKit) usado pelos apps.", "iCloud (CloudKit) cache used by apps."),
        dono: t("macOS (iCloud)", "macOS (iCloud)"),
        nivel: "check_first",
        perde: t("Dados que ainda não sincronizaram, se apagar no meio da sincronização.", "Data that has not synced yet, if deleted while syncing."),
        passo: t("Não apague com o iCloud sincronizando; em geral, deixe como está.", "Do not delete while iCloud is syncing; usually leave it."),
        fonte: CONDICIONAL,
        ..B
    },
    Ficha {
        id: "claude_vm_bundles",
        casa: &[Caminho("~/Library/Application Support/Claude/vm_bundles")],
        o_que_e: t("Máquina virtual do app Claude.", "The Claude app's virtual machine."),
        dono: t("O app Claude.", "The Claude app."),
        sozinho: Sozinho::Nao,
        nivel: "check_first",
        perde: t("A máquina virtual; ela se corrompe se mexer com o app aberto.", "The virtual machine; it gets corrupted if touched while the app is open."),
        passo: t("Feche o app Claude antes: é a máquina virtual dele, e mexer com ela em uso a corrompe.", "Quit the Claude app first; this is its virtual machine."),
        agir: &[Acao::AppDono],
        fonte: CONDICIONAL,
        ..B
    },
    Ficha {
        id: "preferences_anomaly",
        casa: &[Caminho("~/Library/Preferences/*")],
        o_que_e: t("Arquivo ou pasta grande dentro de Preferences, que deveria ter poucos KB.", "A large file or folder inside Preferences, which should hold a few KB."),
        dono: t("Um app; procure qual.", "An app; find out which."),
        nivel: "anomaly",
        perde: t("As preferências do app dono.", "The owning app's preferences."),
        passo: t("Anomalia: a pasta Preferences deveria ter poucos KB. Procure o app dono antes de mexer.", "Anomaly: Preferences should hold kilobytes; look for the owning app."),
        agir: &[Acao::Finder],
        fonte: &[Ref::Interno("docs/04-catalogo-de-alvos.md Anomalias")],
        ..B
    },
    Ficha {
        id: "icloud_drive",
        casa: &[Caminho("~/Library/Mobile Documents/*")],
        o_que_e: t("Seus arquivos do iCloud Drive.", "Your iCloud Drive files."),
        dono: t("Você e o iCloud.", "You and iCloud."),
        sozinho: Sozinho::Quando(t("com Otimizar Armazenamento do Mac ligado, o macOS tira as cópias locais que já estão no iCloud", "with Optimize Mac Storage on, macOS removes local copies that are already in iCloud")),
        nivel: "do_not_touch",
        perde: t("Os arquivos, em todos os aparelhos.", "The files, on every device."),
        passo: t("Não apague: é o iCloud Drive, e os arquivos sumiriam de todos os aparelhos. Para liberar espaço, use “Otimizar Armazenamento do Mac”.", "Do not delete: this is iCloud Drive and it would disappear from every device. Use Optimize Mac Storage."),
        agir: &[Acao::Ajustes(Painel::Armazenamento)],
        fonte: PROIBIDO,
        ..B
    },
    Ficha {
        id: "cloud_storage",
        casa: &[Caminho("~/Library/CloudStorage/*")],
        o_que_e: t("Arquivos do Google Drive, Dropbox e afins.", "Google Drive, Dropbox and similar files."),
        dono: t("O app do provedor.", "The provider's app."),
        nivel: "do_not_touch",
        perde: t("Arquivos reais.", "Real files."),
        passo: t("São dados reais; libere espaço pelo app do provedor.", "Real data; free space through the provider's app."),
        agir: &[Acao::AppDono],
        fonte: PROIBIDO,
        ..B
    },
    Ficha {
        id: "asset_voices",
        casa: &[asset!("MacinTalkVoiceAssets"), asset!("VoiceServices_*"), asset!("TTSAXResourceModelAssets"), asset!("Trial_Siri_SiriTextToSpeech")],
        o_que_e: t("Vozes do sistema, baixadas pelo macOS.", "System voices, downloaded by macOS."),
        nivel: "cost",
        perde: t("As vozes removidas, até baixarem de novo.", "The removed voices, until they download again."),
        passo: t("Vozes do sistema. Remova vozes em Ajustes > Acessibilidade > Conteúdo Falado; não apague dentro de AssetsV2.", "System voices. Remove voices in System Settings > Accessibility > Spoken Content; do not delete inside AssetsV2."),
        fonte: F6,
        ..B
    },
    Ficha {
        id: "asset_xcode",
        casa: &[asset!("AppleDeveloperDocumentation"), asset!("MetalToolchain")],
        o_que_e: t("Documentação e ferramentas Metal do Xcode, baixadas pelo macOS.", "Xcode documentation and Metal tools, downloaded by macOS."),
        dono: t("Xcode", "Xcode"),
        nivel: "cost",
        perde: BAIXA,
        passo: t("Documentação e ferramentas Metal do Xcode. Veja em Xcode > Settings > Components; não apague dentro de AssetsV2.", "Xcode documentation and Metal tools. See Xcode > Settings > Components; do not delete inside AssetsV2."),
        agir: &[Acao::AppDono],
        fonte: F6,
        ..B
    },
    Ficha {
        id: "asset_sim_runtime",
        casa: &[asset!("*SimulatorRuntime")],
        o_que_e: t("Runtimes de simulador baixados pelo macOS.", "Simulator runtimes downloaded by macOS."),
        dono: t("Xcode", "Xcode"),
        nivel: "cost",
        perde: BAIXA,
        passo: t("Runtimes de simulador baixados pelo macOS. Veja com xcrun simctl runtime list; remova em Xcode > Settings > Components.", "Simulator runtimes downloaded by macOS. See them with xcrun simctl runtime list; remove them in Xcode > Settings > Components."),
        agir: &[Acao::Comando("xcrun simctl runtime list"), Acao::AppDono],
        fonte: F6,
        ..B
    },
    Ficha {
        id: "asset_os_update",
        casa: &[asset!("MacSoftwareUpdate"), asset!("MacSplatSoftwareUpdate"), asset!("*UpdateBrain"), asset!("SFRSoftwareUpdate")],
        o_que_e: t("Atualização do macOS já baixada.", "A macOS update already downloaded."),
        sozinho: Sozinho::Quando(t("depois de instalar a atualização", "after the update is installed")),
        perde: t("O download da atualização, que baixa de novo.", "The update download, which downloads again."),
        passo: t("Atualização do macOS baixada. Instale ou aguarde em Ajustes > Geral > Atualização de Software; o macOS a remove depois de instalar.", "A macOS update was downloaded. Install it or wait in System Settings > General > Software Update; macOS removes it after installing."),
        fonte: F6,
        ..B
    },
    Ficha {
        id: "spotlight_index_large",
        casa: &[Caminho("/.Spotlight-V100"), Caminho("~/Library/Metadata/CoreSpotlight")],
        o_que_e: t("O índice do Spotlight, que permite buscar arquivos e o conteúdo deles.", "The Spotlight index, which lets you search files and their contents."),
        dono: t("macOS (Spotlight)", "macOS (Spotlight)"),
        nivel: "check_first",
        perde: t("A busca do Spotlight até o índice ser refeito.", "Spotlight search until the index is rebuilt."),
        passo: t("O índice do Spotlight está grande. Não apague à mão; veja em Ajustes > Spotlight.", "The Spotlight index is large. Do not delete it by hand; see System Settings > Spotlight."),
        agir: &[Acao::Ajustes(Painel::Spotlight)],
        fonte: F6,
        ..B
    },
    Ficha {
        id: "failed_smb_mount",
        casa: &[Caminho("/Volumes")],
        o_que_e: t("Dados numa pasta comum dentro de /Volumes, no próprio volume Data. Ali deveria haver só pontos de montagem; os discos e compartilhamentos montados não entram nesta conta.", "Data in a regular folder inside /Volumes, on the Data volume itself. There should only be mount points there; mounted disks and shares are not in this count."),
        dono: t("Quem gravou ali; abra no Finder para ver.", "Whoever wrote there; open it in Finder to see."),
        sozinho: Sozinho::Nao,
        nivel: "anomaly",
        perde: t("O que estiver gravado ali; confira antes.", "Whatever is written there; check first."),
        passo: t("Há dados gravados no próprio volume Data, numa pasta comum dentro de /Volumes, onde deveria haver só pontos de montagem. Não é o disco nem o compartilhamento: abra no Finder e confira o que é.", "There is data written to the Data volume itself, in a regular folder inside /Volumes, where there should only be mount points. It is not the disk or the share: open it in Finder and check what it is."),
        agir: &[Acao::Finder],
        fonte: F6,
        ..B
    },
    // ---- sem regra: só dão `card` a linhas e seções (§5.1) ----
    Ficha {
        id: "svc_time_machine",
        casa: &[Servico("com.apple.TMCacheDelete")],
        o_que_e: t("Snapshots locais do Time Machine, vistos pelo serviço que os apaga quando falta espaço. São os mesmos bytes da linha Snapshots.", "Local Time Machine snapshots, as seen by the service that deletes them when space runs low. They are the same bytes as the Snapshots line."),
        dono: t("macOS (Time Machine)", "macOS (Time Machine)"),
        sozinho: FALTA,
        perde: t("O ponto de restauração local.", "The local restore point."),
        ..B
    },
    Ficha {
        id: "svc_fspurgeable",
        casa: &[Servico("com.apple.fspurgeable_data"), Servico("com.apple.fspurgeable_document")],
        o_que_e: t("Arquivos que o macOS ou um app marcou como removíveis.", "Files macOS or an app marked as removable."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "svc_spotlight_cache",
        casa: &[Servico("com.apple.metadata.mds.cachedelete")],
        o_que_e: t("Caches do Spotlight.", "Spotlight caches."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "svc_app_container_caches",
        casa: &[Servico("com.apple.cache_delete_app_container_caches")],
        o_que_e: t("Caches dentro dos contêineres dos apps.", "Caches inside app containers."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "svc_photos",
        casa: &[Servico("com.apple.photolibraryd.cacheDelete")],
        o_que_e: t("Cópias e miniaturas do Fotos que voltam do iCloud quando preciso.", "Photos copies and thumbnails that come back from iCloud when needed."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "svc_mobileasset",
        casa: &[Servico("com.apple.mobileassetd.cache-delete")],
        o_que_e: t("Recursos baixados pelo macOS que ele mesmo pode baixar de novo.", "Resources downloaded by macOS that it can download again itself."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "svc_thumbnails",
        casa: &[Servico("com.apple.quicklook.ThumbnailsAgent.CacheDelete")],
        o_que_e: t("Miniaturas do Quick Look.", "Quick Look thumbnails."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "svc_asset_cache",
        casa: &[Servico("com.apple.AssetCache.builtin.CacheDelete")],
        o_que_e: t("Cache de Conteúdo do macOS.", "macOS Content Caching."),
        sozinho: FALTA,
        ..B
    },
    Ficha {
        id: "vol_system",
        casa: &[Volume("System")],
        o_que_e: t("O macOS em si, selado e só leitura.", "macOS itself, sealed and read-only."),
        sozinho: Sozinho::Nao,
        nivel: "do_not_touch",
        perde: t("O funcionamento do macOS.", "macOS itself stops working."),
        passo: NAO_MEXER,
        ..B
    },
    Ficha {
        id: "vol_preboot",
        casa: &[Volume("Preboot")],
        o_que_e: t("Arquivos para ligar o Mac e os cryptexes do sistema.", "Files to start up the Mac and the system cryptexes."),
        sozinho: Sozinho::Nao,
        nivel: "do_not_touch",
        perde: t("A inicialização do Mac.", "The Mac's ability to start up."),
        passo: NAO_MEXER,
        ..B
    },
    Ficha {
        id: "vol_recovery",
        casa: &[Volume("Recovery")],
        o_que_e: t("O sistema de recuperação.", "The recovery system."),
        sozinho: Sozinho::Nao,
        nivel: "do_not_touch",
        perde: t("A recuperação do Mac.", "The Mac's recovery."),
        passo: NAO_MEXER,
        ..B
    },
    Ficha {
        id: "vol_vm",
        casa: &[Volume("VM")],
        o_que_e: t("Arquivos de swap (memória que foi parar no disco) e a imagem do kernel.", "Swap files (memory that ended up on disk) and the kernel image."),
        sozinho: Sozinho::Quando(t("o swap sai ao reiniciar", "swap goes away on restart")),
        perde: t("Nada; ao reiniciar, o trabalho aberto que não foi salvo.", "Nothing; on restart, open work that was not saved."),
        passo: t("Fechar apps pesados ou reiniciar devolve o swap.", "Quitting heavy apps or restarting gives the swap back."),
        agir: &[Acao::Reiniciar],
        ..B
    },
    Ficha {
        id: "vol_update",
        casa: &[Volume("Update")],
        o_que_e: t("Área de trabalho das atualizações do macOS.", "Working area for macOS updates."),
        sozinho: Sozinho::Quando(t("ao terminar a atualização", "when the update finishes")),
        ..B
    },
    Ficha {
        id: "term_snapshots",
        casa: &[Termo("snapshots")],
        o_que_e: t("Blocos que só existem em snapshots locais.", "Blocks that exist only in local snapshots."),
        sozinho: Sozinho::Quando(t("depende do snapshot", "depends on the snapshot")),
        perde: t("Depende do snapshot: o ponto de restauração que ele guarda.", "Depends on the snapshot: the restore point it keeps."),
        passo: t("Veja o raio-X dos snapshots.", "See the snapshot breakdown."),
        fonte: MAPA3,
        ..B
    },
    Ficha {
        id: "term_large_xattrs",
        casa: &[Termo("large_xattrs")],
        o_que_e: t("Atributos estendidos grandes, guardados fora do arquivo.", "Large extended attributes, stored outside the file."),
        dono: t("Os apps que os gravam nos seus arquivos.", "The apps that write them to your files."),
        sozinho: Sozinho::Nao,
        nivel: "do_not_touch",
        perde: t("Informações que os apps guardam junto dos arquivos.", "Information apps keep alongside the files."),
        fonte: MAPA3,
        ..B
    },
    Ficha {
        id: "asset_code_model",
        casa: &[asset!("UAF_FM_CodeLM")],
        o_que_e: t("Modelo da Apple para código (pelo nome da pasta).", "Apple model for code (by the folder name)."),
        passo: t("Gerenciado pelo macOS. Não apague dentro de AssetsV2.", "Managed by macOS. Do not delete inside AssetsV2."),
        ..B
    },
    Ficha {
        id: "asset_siri_speech",
        casa: &[asset!("UAF_Siri_*"), asset!("UAF_Speech_*"), asset!("VoiceTrigger*"), asset!("SpeechEndpoint*")],
        o_que_e: t("Siri e ditado.", "Siri and dictation."),
        passo: t("Gerenciado pelo macOS. Veja em Ajustes > Siri.", "Managed by macOS. See System Settings > Siri."),
        ..B
    },
    Ficha {
        id: "asset_language",
        casa: &[asset!("UAF_LinguisticData"), asset!("LinguisticData")],
        o_que_e: t("Dados de idioma.", "Language data."),
        passo: GERENCIADO,
        ..B
    },
    Ficha {
        id: "asset_dictionaries",
        casa: &[asset!("DictionaryServices_*")],
        o_que_e: t("Dicionários.", "Dictionaries."),
        passo: t("Escolha os dicionários nos ajustes do app Dicionário.", "Choose dictionaries in the Dictionary app settings."),
        agir: &[Acao::AppDono],
        ..B
    },
    Ficha {
        id: "asset_photos",
        casa: &[asset!("UAF_Photos_*"), asset!("VCPMobileAssets"), asset!("ImageCaptionModel"), asset!("PromptBasedSegmentation"), asset!("PhotosCuratedMusicLibraryAsset")],
        o_que_e: t("Análise e efeitos do Fotos.", "Photos analysis and effects."),
        passo: GERENCIADO,
        ..B
    },
    Ficha {
        id: "asset_translation",
        casa: &[asset!("UAF_Translation_*"), asset!("SpeechTranslationAssets*")],
        o_que_e: t("Idiomas de tradução.", "Translation languages."),
        passo: t("Veja em Ajustes > Geral > Idioma e Região.", "See System Settings > General > Language & Region."),
        ..B
    },
    // ---- fichas do Início (spec-inicio §9.3): só dão `card` às linhas; refs abertas e conferidas ----
    Ficha {
        id: "other_users",
        casa: &[Caminho("/Users/*")],
        o_que_e: t("Pasta de outra conta deste Mac, a pasta Compartilhada ou uma conta apagada guardada em imagem de disco.", "Another account's folder on this Mac, the Shared folder, or a deleted account kept as a disk image."),
        dono: t("Quem usa a conta, ou um administrador deste Mac.", "Whoever uses the account, or an administrator of this Mac."),
        nivel: "check_first",
        perde: t("Arquivos de outras contas.", "Files of other accounts."),
        passo: t("Pasta de outra conta, a Compartilhada ou uma conta apagada. Quem usa a pasta, ou um administrador em Ajustes > Usuários e Grupos, decide o que fica.", "Another account's folder, the Shared folder or a deleted account. Whoever uses it, or an administrator in System Settings > Users & Groups, decides what stays."),
        // "This folder contains the home folders of all the people set up as users on your Mac";
        // "The disk image is saved in /Users/Deleted Users/".
        fonte: &[Ref::Apple("https://support.apple.com/guide/mac-help/folders-that-come-with-your-mac-mchlp1143/mac"), Ref::Apple("https://support.apple.com/guide/mac-help/delete-a-user-or-group-mchlp1557/mac")],
        ..B
    },
    Ficha {
        id: "user_folder",
        casa: &[Caminho("~/Desktop"), Caminho("~/Documents"), Caminho("~/Movies"), Caminho("~/Music"), Caminho("~/Pictures"), Caminho("~/Downloads")],
        o_que_e: t("Pasta sua que veio com o Mac: Mesa, Documentos, Downloads, Filmes, Música ou Imagens.", "A folder of yours that came with the Mac: Desktop, Documents, Downloads, Movies, Music or Pictures."),
        dono: VOCE,
        nivel: "check_first",
        perde: t("Os arquivos seus que você apagar.", "Your files that you delete."),
        passo: t("Pasta sua. Veja o que pesa em Pastas maiores antes de mexer.", "Your folder. Look at what takes the space inside it before changing anything."),
        agir: &[Acao::Finder],
        // "has folders for your desktop files, downloads, pictures, documents, movies, music".
        fonte: &[Ref::Apple("https://support.apple.com/guide/mac-help/folders-that-come-with-your-mac-mchlp1143/mac")],
        ..B
    },
    Ficha {
        id: "container_vm",
        // Do OrbStack, o app group e não o ~/.orbstack (ferramentas): é lá que fica o data.img.
        casa: &[Caminho("~/.colima"), Caminho("~/.lima"), Caminho("~/Library/Containers/com.docker.docker"), Caminho("~/Library/Group Containers/HUAQ24HBR6.dev.orbstack")],
        o_que_e: t("Disco da máquina virtual Linux do Docker Desktop, Colima, Lima ou OrbStack, onde ficam os contêineres e as imagens.", "Disk of the Linux virtual machine of Docker Desktop, Colima, Lima or OrbStack, where the containers and images live."),
        dono: t("O programa que criou a máquina (Docker Desktop, Colima, Lima ou OrbStack).", "The program that created the machine (Docker Desktop, Colima, Lima or OrbStack)."),
        sozinho: Sozinho::Nao,
        nivel: "check_first",
        perde: t("Os contêineres, imagens, volumes ou máquinas que você apagar.", "The containers, images, volumes or machines you delete."),
        passo: t("Reduza pelo próprio programa, apagando lá dentro o que não usa: contêineres, imagens, volumes ou máquinas. Apagar a pasta por fora apaga a máquina inteira, com tudo dentro. No Colima, o espaço volta ao Mac quando a máquina reinicia.", "Shrink it through the program itself by deleting what you do not use inside it: containers, images, volumes or machines. Deleting the folder from outside deletes the whole machine and everything in it. In Colima, the space returns to the Mac when the machine restarts."),
        agir: &[Acao::AppDono],
        // Docker: "stores Linux containers and images in a single, large disk image file"; Colima:
        // "Disk space can be freed in the VM by removing containers"; Lima: o disco fica no
        // diretório da instância; OrbStack: o data.img "automatically shrinks when you delete data".
        fonte: &[
            Ref::Relato("https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/"),
            Ref::Relato("https://github.com/abiosoft/colima/blob/main/docs/FAQ.md"),
            Ref::Relato("https://lima-vm.io/docs/dev/internals/"),
            Ref::Relato("https://docs.orbstack.dev/faq"),
        ],
        ..B
    },
    Ficha {
        id: "whatsapp_data",
        casa: &[Caminho("~/Library/Group Containers/group.net.whatsapp.WhatsApp.shared")],
        o_que_e: t("Pasta compartilhada (app group) do WhatsApp para Mac, onde o app grava os dados dele.", "Shared folder (app group) of WhatsApp for Mac, where the app writes its data."),
        dono: t("O app WhatsApp.", "The WhatsApp app."),
        nivel: "check_first",
        perde: t("O que o WhatsApp gravou neste Mac.", "What WhatsApp wrote on this Mac."),
        passo: t("O WhatsApp para Mac não tem a função de liberar espaço que o app do celular tem. A pasta é do app: confira antes de mexer.", "WhatsApp for Mac does not have the free-up-storage feature the phone app has. The folder belongs to the app: check before touching it."),
        // WhatsApp: "This feature isn't available on the Mac app"; Apple: o app group fica em
        // ~/Library/Group Containers/<application-group-id>.
        fonte: &[
            Ref::Relato("https://faq.whatsapp.com/5503646096388294/?cms_platform=mac-desktop"),
            Ref::Apple("https://developer.apple.com/documentation/foundation/filemanager/containerurl(forsecurityapplicationgroupidentifier:)"),
        ],
        ..B
    },
    Ficha {
        id: "browser_cache",
        casa: &[Caminho("~/Library/Caches/Google/Chrome"), Caminho("~/Library/Caches/com.apple.Safari"), Caminho("~/Library/Caches/Firefox")],
        o_que_e: t("Cache do navegador (Chrome, Safari ou Firefox): partes de páginas, como imagens, guardadas para abrir mais rápido.", "Browser cache (Chrome, Safari or Firefox): parts of pages, such as images, kept so they open faster."),
        dono: t("O navegador.", "The browser."),
        nivel: "cost",
        perde: t("Alguns sites abrem mais devagar até as imagens carregarem de novo; no Safari, também pode sair das contas dos sites.", "Some sites open more slowly until images load again; in Safari it may also sign you out of websites."),
        passo: t("Apague só o cache pelo próprio navegador: no Chrome, em Mais > Excluir dados de navegação; no Firefox, em Privacidade e segurança. No Safari, Ajustes > Privacidade > Gerenciar Dados de Sites também apaga os dados dos sites.", "Delete only the cache in the browser itself: in Chrome, More > Delete browsing data; in Firefox, the Privacy and security settings. In Safari, Settings > Privacy > Manage Website Data also removes website data."),
        agir: &[Acao::AppDono],
        // Google: "The cache remembers parts of pages, like images"; Apple: "may also sign you out
        // of websites"; Mozilla: só "Temporary cached files and pages".
        fonte: &[
            Ref::Relato("https://support.google.com/accounts/answer/32050"),
            Ref::Apple("https://support.apple.com/guide/safari/manage-cookies-sfri11471/mac"),
            Ref::Relato("https://support.mozilla.org/en-US/kb/how-clear-firefox-cache"),
        ],
        ..B
    },
    // Por último entre os caminhos: pega o que sobrou de /System (§6.1).
    Ficha {
        id: "system_folders",
        casa: &[Caminho("/System"), Caminho("/private/var/db"), Caminho("/private/var/vm")],
        o_que_e: t("Pastas do sistema e pastas que o macOS protege.", "System folders and folders macOS protects."),
        sozinho: Sozinho::Nao,
        nivel: "do_not_touch",
        perde: t("O funcionamento do macOS.", "macOS itself stops working."),
        passo: t("Não mexer: é do sistema; o macOS precisa dessas pastas.", "Do not touch: it belongs to the system; macOS needs these folders."),
        fonte: PROIBIDO,
        ..B
    },
];

const fn c(checked: &'static str, on: &'static [&'static str], at: &'static str, apple_command: Option<&'static str>) -> Conf {
    Conf { checked, on, at, apple_command }
}

const TRES: &[&str] = &["15", "26", "27"];
const SO27: &[&str] = &["27"];
const MAPA: &str = "2026-10-01";

/// Cada `source` do JSON e como foi conferida contra a Apple (§4.6). `at` vazio: não conferida.
pub const FONTES: &[(&str, Conf)] = &[
    ("diskutil_capacity_in_use", c("byte", TRES, MAPA, Some("diskutil apfs list -plist"))),
    ("settings_ui_log", c("byte", SO27, MAPA, Some("/usr/bin/log show --last 1h --predicate 'subsystem == \"com.apple.settings.Storage\"'"))),
    ("attr_vol_spaceused", c("byte", TRES, MAPA, Some("df -k /System/Volumes/Data"))),
    ("statfs", c("byte", SO27, MAPA, Some("df -k /"))),
    ("cachedelete", c("byte", SO27, MAPA, None)),
    ("fsctl_dir_stats", c("byte", TRES, MAPA, None)),
    ("dir_stats_like_settings", c("byte", SO27, MAPA, None)),
    // Nenhum comando da Apple mostra esse número.
    ("fsctl_snapshot_private_size", c("margin", SO27, MAPA, None)),
    ("getattrlistbulk", c("margin", TRES, MAPA, None)),
    // É o número do serviço, não o da tela (mapa-de-fontes §6.1).
    ("storage_management_log", c("no", &[], "", None)),
    ("listxattr", c("no", &[], "", None)),
    // Vale só para o lado lógico.
    ("lsof_proc_pidfdinfo", c("no", &[], "", Some("lsof -b -w +L1"))),
    ("sysctl_vm_swapusage", c("no", &[], "", Some("sysctl vm.swapusage"))),
    ("sysctl_kern_boottime", c("no", &[], "", Some("sysctl kern.boottime"))),
    ("diskutil_list_snapshots", c("no", &[], "", Some("diskutil apfs listSnapshots /System/Volumes/Data"))),
    ("tm_preferences_plist", c("no", &[], "", None)),
    // Lixo > 30 dias (ATTR_CMN_ADDEDTIME) e metadados do APFS (seletor 0x1e): nenhum comando da Apple mostra o número.
    ("getattrlist_addedtime", c("no", &[], "", None)),
    ("apfs_user_client", c("no", &[], "", None)),
    ("difference", c("n/a", &[], "", None)),
    // Categorias do Ajustes reproduzidas (Fase 2): conferidas contra as funções da Apple no macOS 27 (frenteq).
    // Σ dos baldes da travessia pelas regras da Apple; Aplicativos +0,05 %, Documentos 0,0 %, Fotos +1,86 %.
    ("tree_buckets", c("margin", SO27, "2026-10-18", None)),
    // Mesmo número da Apple, ao byte (leitura do Library.musicdb).
    ("music_library_db", c("byte", SO27, "2026-10-18", None)),
    ("messages_db", c("byte", SO27, "2026-10-18", None)),
    // Só conferidos com biblioteca vazia ou ausente: com itens, a Apple não foi comparada.
    ("tv_library_db", c("no", &[], "", None)),
    ("library_presence", c("no", &[], "", None)),
    ("not_reproduced", c("n/a", &[], "", None)),
];

/// Os `apple_command` de `FONTES`, numa lista fixa (o app tem a mesma, `comandosVerificar`).
/// Separada de `conselho::COMANDOS`: estes só leem, para conferir um número.
pub const COMANDOS_VERIFICAR: &[&str] = &[
    "diskutil apfs list -plist",
    "/usr/bin/log show --last 1h --predicate 'subsystem == \"com.apple.settings.Storage\"'",
    "df -k /System/Volumes/Data",
    "df -k /",
    "lsof -b -w +L1",
    "sysctl vm.swapusage",
    "sysctl kern.boottime",
    "diskutil apfs listSnapshots /System/Volumes/Data",
];

pub fn ficha(id: &str) -> Option<&'static Ficha> {
    CATALOGO.iter().find(|f| f.id == id)
}

fn achar(f: impl Fn(&Casa) -> bool) -> Option<&'static str> {
    CATALOGO.iter().find(|x| x.casa.iter().any(&f)).map(|x| x.id)
}

/// Primeira ficha cuja `Caminho` casa (componente inteiro, como as regras). Linha dobrada perde o
/// sufixo antes.
pub fn card_de_caminho(p: &str) -> Option<&'static str> {
    let p = p.strip_suffix(DOBRADO).unwrap_or(p);
    achar(|c| matches!(c, Casa::Caminho(x) if casa(x, p)))
}

fn card_de_termo(k: &str) -> Option<&'static str> {
    achar(|c| matches!(c, Casa::Termo(x) if *x == k))
}

/// Igualdade, ou prefixo quando o nome tem 15+ caracteres (o `p_comm` vem cortado).
fn card_de_processo(n: &str) -> Option<&'static str> {
    achar(|c| matches!(c, Casa::Processo(x) if *x == n || (n.len() >= 15 && x.starts_with(n))))
}

fn card_de_servico(n: &str) -> Option<&'static str> {
    achar(|c| matches!(c, Casa::Servico(x) if *x == n))
}

fn card_de_volume(roles: &Value) -> Option<&'static str> {
    let r: Vec<&str> = roles.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    achar(|c| matches!(c, Casa::Volume(x) if r.contains(x)))
}

/// `card` em cada objeto da lista em `ptr`; lista ausente é ignorada.
fn cada(v: &mut Value, ptr: &str, card: impl Fn(&Value) -> Option<&'static str>) {
    for x in v.pointer_mut(ptr).and_then(Value::as_array_mut).into_iter().flatten().filter(|x| x.is_object()) {
        let c = card(x);
        x["card"] = json!(c);
    }
}

fn citados(v: &Value, out: &mut Vec<&'static str>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter().filter(|(k, _)| *k != "cards") {
                match (k.as_str(), x.as_str().and_then(ficha)) {
                    ("card", Some(f)) => out.push(f.id),
                    _ => citados(x, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| citados(x, out)),
        _ => {}
    }
}

/// Preenche `card` em `top[]`, `top_services[]` (completo e rápido), `containers[].volumes[]`,
/// `deleted_open_processes[]`, `snapshots[]`, `data_volume.terms[]` e no `why`; a linha do Ajustes
/// em `top[]` (`linhas`); depois `cards` com as fichas citadas (inclusive pelo `advice`). Idempotente.
pub fn anexar(v: &mut Value) {
    if !v.is_object() {
        return;
    }
    cada(v, "/top", |x| match x["path"].as_str() {
        Some(p) => card_de_caminho(p),
        // Linha sem caminho (snapshots, apagados): a ficha do termo.
        None => x["term"].as_str().and_then(card_de_termo),
    });
    // `home` só no completo (o bin põe antes do `anexar`): resolve o `~` do rótulo do File Provider.
    let home = v["home"].as_str().map(str::to_string);
    for x in v.pointer_mut("/top").and_then(Value::as_array_mut).into_iter().flatten().filter(|x| x.is_object()) {
        let (l, g) = linhas::linha(x["path"].as_str(), x["term"].as_str().unwrap_or(""));
        x["settings_line"] = json!(l);
        x["settings_line_grade"] = json!(g);
        if let (Some(h), Some(p), false) = (&home, x["path"].as_str(), x["folded"] == true)
            && let Some(r) = linhas::rotulo(p, h)
        {
            x["label"] = json!(r);
        }
    }
    // Só no completo (precisa de `home`); ler Info.plist e xattr, nunca escreve.
    crate::owners::anexar(v);
    // Depois das linhas do Ajustes: `system_data.by_line` é a soma delas.
    if let Some(l) = categorias::por_linha(v) {
        if !v["system_data"].is_object() {
            v["system_data"] = json!({});
        }
        v["system_data"]["by_line"] = l;
    }
    for p in ["/informational/purgeable/top_services", "/purgeable/top_services"] {
        cada(v, p, |x| x["name"].as_str().and_then(card_de_servico));
    }
    for i in 0..v["containers"].as_array().map_or(0, Vec::len) {
        cada(v, &format!("/containers/{i}/volumes"), |x| card_de_volume(&x["roles"]));
    }
    cada(v, "/informational/deleted_open_processes", |x| x["name"].as_str().and_then(card_de_processo));
    // Sem `Casa` de snapshot (§0): o tipo já diz qual regra fala dele.
    cada(v, "/informational/snapshots", |x| match x["kind"].as_str() {
        Some("time_machine") => Some("time_machine_snapshots"),
        Some("other") => Some("other_snapshots"),
        _ => None,
    });
    cada(v, "/data_volume/terms", |x| x["key"].as_str().and_then(card_de_termo));
    if v["kind"] == "item" {
        let c = v["display"].as_str().and_then(card_de_caminho);
        v["card"] = json!(c);
    }
    let mut ids = Vec::new();
    citados(v, &mut ids);
    v["cards"] = Value::Object(ids.into_iter().filter_map(ficha).map(|f| (f.id.to_string(), ficha_json(f))).collect::<Map<_, _>>());
}

fn txt(t: &Txt) -> Value {
    crate::traducao::json(t.pt, t.en)
}

pub fn ficha_json(f: &Ficha) -> Value {
    let (sozinho, texto) = match &f.sozinho {
        Sozinho::Sim(t) => ("yes", txt(t)),
        Sozinho::Quando(t) => ("when", txt(t)),
        Sozinho::Nao => ("no", Value::Null),
        Sozinho::NaoInformado => ("unknown", Value::Null),
    };
    json!({
        "what": txt(&f.o_que_e),
        "owner": txt(&f.dono),
        "self_cleans": sozinho,
        "self_cleans_text": texto,
        "level": f.nivel,
        "loses": txt(&f.perde),
        "step": txt(&f.passo),
        "actions": f.agir.iter().map(|a| {
            let (kind, target, command) = match a {
                Acao::Finder => ("finder", None, None),
                Acao::Ajustes(p) => ("settings", Some(match p {
                    Painel::Armazenamento => "storage",
                    Painel::TimeMachine => "time_machine",
                    Painel::Spotlight => "spotlight",
                }), None),
                Acao::AppDono => ("owner_app", None, None),
                Acao::Comando(c) => ("command", None, Some(*c)),
                Acao::Reiniciar => ("restart", None, None),
                Acao::EncerrarSessao => ("log_out", None, None),
                Acao::Nada => ("none", None, None),
            };
            json!({"kind": kind, "target": target, "command": command})
        }).collect::<Vec<_>>(),
        "refs": f.fonte.iter().map(|r| {
            let (kind, s) = match r {
                Ref::Interno(s) => ("internal", s),
                Ref::Apple(s) => ("apple", s),
                Ref::Relato(s) => ("report", s),
                Ref::Medido(s) => ("measured", s),
            };
            json!({"kind": kind, "ref": s})
        }).collect::<Vec<_>>(),
        "verified_on": f.conferido,
    })
}

/// `sdtally --catalog --json`: todas as fichas e a conferência de cada fonte.
pub fn json() -> Value {
    json!({
        "schema_version": 1,
        "kind": "catalog",
        "cards": CATALOGO.iter().map(|f| (f.id.to_string(), ficha_json(f))).collect::<Map<_, _>>(),
        "sources": FONTES.iter().map(|(s, k)| (s.to_string(), json!({
            "checked": k.checked,
            "checked_on": k.on,
            "checked_at": (!k.at.is_empty()).then_some(k.at),
            "apple_command": k.apple_command,
        }))).collect::<Map<_, _>>(),
    })
}
