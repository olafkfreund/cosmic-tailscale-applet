use crate::config::TailscaleConfig;
use crate::fl;
use crate::logic::{
  clear_status, enable_exit_node, exit_node_allow_lan_access, fetch_tailscale_state,
  set_exit_node, set_routes, set_ssh, switch_accounts, tailscale_int_up,
  tailscale_receive, tailscale_send, TailscaleState,
};
use cosmic::app::Core;
use cosmic::cosmic_config::{Config, CosmicConfigEntry};
use cosmic::dialog::file_chooser::{self, FileFilter};
use cosmic::iced::{
  alignment::Horizontal,
  platform_specific::shell::commands::popup::{destroy_popup, get_popup},
  widget::{column, horizontal_space, row},
  window::Id,
  Alignment, Length, Limits,
};
use cosmic::iced_runtime::core::window;
use cosmic::iced_widget::Row;
use cosmic::widget::{
  button, dropdown, list_column,
  settings::{self},
  text, toggler,
};
use cosmic::{Action, Element, Task};
use std::path::PathBuf;
use tracing::{error, warn};
use url::Url;

const ID: &str = "com.github.bhh32.GUIScaleApplet";
const DEFAULT_EXIT_NODE: &str = "Select Exit Node";
const POPUP_MAX_WIDTH: f32 = 720.0;
const POPUP_MIN_WIDTH: f32 = 640.0;
const POPUP_MAX_HEIGHT: f32 = 1080.0;
const POPUP_MIN_HEIGHT: f32 = 200.0;
const STATUS_CLEAR_TIME: u64 = 5;

/// Holds the applet's state
#[allow(clippy::struct_excessive_bools)]
pub struct Window {
  core: Core,
  config: TailscaleConfig,
  config_handler: Option<Config>,
  popup: Option<Id>,
  ssh: bool,
  routes: bool,
  connect: bool,
  device_options: Vec<String>,
  selected_device: String,
  selected_device_idx: Option<usize>,
  send_files: Vec<PathBuf>,
  send_file_status: String,
  files_sent: bool,
  receive_file_status: String,
  avail_exit_nodes: Vec<String>,
  sel_exit_node: String,
  sel_exit_node_idx: Option<usize>,
  acct_list: Vec<String>,
  cur_acct: String,
  allow_lan: bool,
  is_exit_node: bool,
  ip: String,
  conn_status: bool,
}

/// Messages to be sent to the Libcosmic Update function
#[derive(Clone, Debug)]
pub enum Message {
  TogglePopup,
  PopupClosed(Id),
  EnableSSH(bool),
  SshSet(bool, bool),
  AcceptRoutes(bool),
  RoutesSet(bool, bool),
  ConnectDisconnect(bool),
  ConnectionSet(bool, bool),
  SwitchAccount(usize),
  DeviceSelected(usize),
  ChooseFiles,
  FilesSelected(Vec<Url>),
  SendFiles,
  FilesSent(Option<String>),
  FileChoosingCancelled,
  ReceiveFiles,
  FilesReceived(String),
  ExitNodeSelected(usize),
  ExitNodeSet(String, usize, bool),
  AllowExitNodeLanAccess(bool),
  LanAccessSet(bool, bool),
  UpdateIsExitNode(bool),
  ExitNodeEnabled(bool, bool),
  ClearTailDropStatus,
  RefreshState,
  StateRefreshed(Box<TailscaleState>),
  RefreshFailed(String),
}

impl Window {
  fn create_popup(&mut self) -> Task<Action<Message>> {
    let new_id = Id::unique();
    self.popup.replace(new_id);

    let Some(main_id) = self.core.main_window_id() else {
      warn!("No main window ID available for popup");
      return Task::none();
    };

    let mut popup_settings =
      self
        .core
        .applet
        .get_popup_settings(main_id, new_id, None, None, None);

    popup_settings.positioner.size_limits = Limits::NONE
      .max_width(POPUP_MAX_WIDTH)
      .min_width(POPUP_MIN_WIDTH)
      .min_height(POPUP_MIN_HEIGHT)
      .max_height(POPUP_MAX_HEIGHT);

    get_popup(popup_settings)
  }
}

impl cosmic::Application for Window {
  type Executor = cosmic::executor::multi::Executor;
  type Flags = ();
  type Message = Message;
  const APP_ID: &'static str = ID;

  fn core(&self) -> &Core {
    &self.core
  }

  fn core_mut(&mut self) -> &mut Core {
    &mut self.core
  }

  fn init(core: Core, _flags: Self::Flags) -> (Window, Task<Action<Self::Message>>) {
    let (config_handler, config) =
      match Config::new(ID, TailscaleConfig::VERSION) {
        Ok(handler) => match TailscaleConfig::get_entry(&handler) {
          Ok(cfg) => (Some(handler), cfg),
          Err((errs, cfg)) => {
            for err in &errs {
              warn!("Config load error: {err}");
            }
            let _ = cfg.write_entry(&handler);
            (Some(handler), cfg)
          }
        },
        Err(e) => {
          error!("Failed to create config handler: {e}");
          (None, TailscaleConfig::default())
        }
      };

    let sel_exit_node_idx = if config.exit_node_idx > 0 {
      Some(config.exit_node_idx)
    } else {
      None
    };

    let window = Window {
      core,
      config: config.clone(),
      config_handler,
      ssh: false,
      routes: false,
      connect: false,
      device_options: vec!["Select".to_string()],
      popup: None,
      selected_device: DEFAULT_EXIT_NODE.to_string(),
      selected_device_idx: Some(0),
      send_files: Vec::new(),
      send_file_status: String::new(),
      files_sent: false,
      receive_file_status: String::new(),
      avail_exit_nodes: vec!["None".to_string()],
      sel_exit_node: DEFAULT_EXIT_NODE.to_string(),
      sel_exit_node_idx,
      acct_list: Vec::new(),
      cur_acct: String::new(),
      allow_lan: config.allow_lan,
      is_exit_node: false,
      ip: fl!("loading"),
      conn_status: false,
    };

    let task = cosmic::task::future(async { Message::RefreshState });
    (window, task)
  }

  fn on_close_requested(&self, id: window::Id) -> Option<Message> {
    Some(Message::PopupClosed(id))
  }

  fn update(&mut self, message: Self::Message) -> Task<Action<Self::Message>> {
    match message {
      Message::RefreshState => {
        return cosmic::task::future(async {
          match fetch_tailscale_state().await {
            Ok(state) => Message::StateRefreshed(Box::new(state)),
            Err(e) => Message::RefreshFailed(e.to_string()),
          }
        });
      }
      Message::StateRefreshed(state) => {
        self.ip = state.ip;
        self.conn_status = state.connected;
        self.connect = state.connected;
        self.ssh = state.ssh_enabled;
        self.routes = state.routes_enabled;
        self.is_exit_node = state.is_exit_node;
        self.device_options = state.devices;
        self.avail_exit_nodes = state.exit_nodes;
        self.acct_list = state.acct_list;
        self.cur_acct = state.current_acct;
      }
      Message::RefreshFailed(err) => {
        error!("Failed to refresh Tailscale state: {err}");
      }
      Message::TogglePopup => {
        return if let Some(p) = self.popup.take() {
          self.receive_file_status = String::new();
          destroy_popup(p)
        } else {
          self.create_popup()
        }
      }
      Message::PopupClosed(id) => {
        if self.popup.as_ref() == Some(&id) {
          self.popup = None;
        }
      }
      Message::EnableSSH(enabled) => {
        self.ssh = enabled;
        let ssh = self.ssh;
        return cosmic::task::future(async move {
          let success = set_ssh(ssh).await.is_ok();
          Message::SshSet(ssh, success)
        });
      }
      Message::SshSet(value, success) => {
        if !success {
          self.ssh = !value;
          error!("Failed to set SSH to {value}");
        }
      }
      Message::AcceptRoutes(accepted) => {
        self.routes = accepted;
        let routes = self.routes;
        return cosmic::task::future(async move {
          let success = set_routes(routes).await.is_ok();
          Message::RoutesSet(routes, success)
        });
      }
      Message::RoutesSet(value, success) => {
        if !success {
          self.routes = !value;
          error!("Failed to set routes to {value}");
        }
      }
      Message::ConnectDisconnect(connection) => {
        self.connect = connection;
        self.conn_status = connection;
        let connect = self.connect;
        return cosmic::task::future(async move {
          let success = tailscale_int_up(connect).await.is_ok();
          Message::ConnectionSet(connect, success)
        });
      }
      Message::ConnectionSet(value, success) => {
        if !success {
          self.connect = !value;
          self.conn_status = !value;
          error!("Failed to set connection to {value}");
        }
      }
      Message::SwitchAccount(new_acct) => {
        if let Some(acct) = self.acct_list.get(new_acct).cloned() {
          self.cur_acct.clone_from(&acct);
          return cosmic::task::future(async move {
            if let Err(e) = switch_accounts(&acct).await {
              error!("Failed to switch accounts: {e}");
            }
            Message::RefreshState
          });
        }
      }
      Message::DeviceSelected(device) => {
        if let Some(dev) = self.device_options.get(device) {
          self.selected_device = dev.clone();
          self.selected_device_idx = Some(device);

          if self.files_sent {
            self.files_sent = false;
          }
        }
      }
      Message::ChooseFiles => {
        return cosmic::task::future(async move {
          let file_filter = FileFilter::new("Any").glob("*.*");
          let dialog = file_chooser::open::Dialog::new()
            .title(fl!("choose-files-title"))
            .filter(file_filter);

          match dialog.open_files().await {
            Ok(file_responses) => {
              Message::FilesSelected(file_responses.urls().to_vec())
            }
            Err(file_chooser::Error::Cancelled) => Message::FileChoosingCancelled,
            Err(e) => {
              error!("Choosing a file or files went wrong: {e}");
              Message::FileChoosingCancelled
            }
          }
        });
      }
      Message::FilesSelected(urls) => {
        for url in &urls {
          if let Ok(path) = url.to_file_path() {
            if path.exists() {
              self.send_files.push(path);
            }
          } else {
            warn!("Invalid file URL: {url}");
          }
        }

        self.files_sent = false;
        return self.create_popup();
      }
      Message::SendFiles => {
        let files = self.send_files.clone();
        let dev = self.selected_device.clone();

        if dev != "Select" {
          self.files_sent = true;
          return cosmic::task::future(async move {
            let tx_status = tailscale_send(&files, &dev).await;
            Message::FilesSent(tx_status)
          });
        }
      }
      Message::FilesSent(tx_status) => {
        self.send_file_status = match tx_status {
          Some(err_val) => err_val,
          None => fl!("files-sent-success"),
        };

        if !self.send_file_status.is_empty() {
          if !self.send_files.is_empty() {
            self.send_files.clear();
          }

          return cosmic::task::future(async move { Message::ClearTailDropStatus });
        }
      }
      Message::FileChoosingCancelled => {
        return self.create_popup();
      }
      Message::ReceiveFiles => {
        return cosmic::task::future(async move {
          let rx_status = tailscale_receive().await;
          Message::FilesReceived(rx_status)
        });
      }
      Message::FilesReceived(rx_status) => {
        self.receive_file_status = rx_status;

        if !self.receive_file_status.is_empty() {
          return cosmic::task::future(async move { Message::ClearTailDropStatus });
        }
      }
      Message::ExitNodeSelected(exit_node) => {
        if !self.is_exit_node
          && let Some(node) = self.avail_exit_nodes.get(exit_node).cloned()
        {
          self.sel_exit_node.clone_from(&node);
          self.sel_exit_node_idx = Some(exit_node);

          let exit_node_name = if exit_node == 0 {
            String::new()
          } else {
            node.clone()
          };

          return cosmic::task::future(async move {
            let success = set_exit_node(&exit_node_name).await.is_ok();
            Message::ExitNodeSet(node, exit_node, success)
          });
        }
      }
      Message::ExitNodeSet(_node, idx, success) => {
        if success {
          if let Some(ref handler) = self.config_handler
            && let Err(e) = self.config.set_exit_node_idx(handler, idx)
          {
            error!("Failed to save exit node config: {e}");
          }
        } else {
          error!("Failed to set exit node");
        }
      }
      Message::AllowExitNodeLanAccess(allow_lan_access) => {
        self.allow_lan = allow_lan_access;

        if self.is_exit_node {
          let allow = self.allow_lan;
          return cosmic::task::future(async move {
            let success = exit_node_allow_lan_access(allow).await.is_ok();
            Message::LanAccessSet(allow, success)
          });
        }
      }
      Message::LanAccessSet(value, success) => {
        if success {
          if let Some(ref handler) = self.config_handler
            && let Err(e) = self.config.set_allow_lan(handler, value)
          {
            error!("Failed to save LAN access config: {e}");
          }
        } else {
          self.allow_lan = !value;
          error!("Failed to set LAN access");
        }
      }
      Message::UpdateIsExitNode(is_exit_node) => {
        if self.sel_exit_node_idx == Some(0) || self.sel_exit_node_idx.is_none() {
          self.is_exit_node = is_exit_node;
          let exit_node = self.is_exit_node;

          return cosmic::task::future(async move {
            let success = enable_exit_node(exit_node).await.is_ok();
            Message::ExitNodeEnabled(exit_node, success)
          });
        }
      }
      Message::ExitNodeEnabled(value, success) => {
        if success {
          return cosmic::task::future(async { Message::RefreshState });
        }
        self.is_exit_node = !value;
        error!("Failed to enable/disable exit node");
      }
      Message::ClearTailDropStatus => {
        if !self.receive_file_status.is_empty() {
          return cosmic::task::future(async move {
            Message::FilesReceived(
              match clear_status(STATUS_CLEAR_TIME).await {
                Some(bad_value) => format!(
                  "Something went wrong and clear status returned a value: {bad_value}"
                ),
                None => String::new(),
              },
            )
          });
        } else if !self.send_file_status.is_empty() || self.files_sent {
          self.selected_device_idx = Some(0);
          if let Some(dev) = self.device_options.first() {
            self.selected_device = dev.clone();
          }

          return cosmic::task::future(async move {
            Message::FilesSent(match clear_status(STATUS_CLEAR_TIME).await {
              Some(bad_value) => Some(format!(
                "Something went wrong and clear status returned a value: {bad_value}"
              )),
              None => Some(String::new()),
            })
          });
        }
      }
    }
    Task::none()
  }

  fn view(&self) -> Element<'_, Self::Message> {
    self
      .core
      .applet
      .icon_button("tailscale-icon")
      .on_press(Message::TogglePopup)
      .into()
  }

  fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
    let cur_acct = &self.cur_acct;
    let acct_list = &self.acct_list;
    let ip = &self.ip;

    let mut sel_acct_idx = None;
    for (idx, acct) in acct_list.iter().enumerate() {
      if acct == cur_acct {
        sel_acct_idx = Some(idx);
        break;
      }
    }

    let conn_status = self.conn_status;

    let status_elements: Vec<Element<'_, Message>> = vec![Element::from(column!(
      row!(settings::item(
        fl!("account"),
        dropdown(acct_list, sel_acct_idx, Message::SwitchAccount)
      )),
      row!(settings::item(
        fl!("tailscale-address"),
        text(ip.clone()),
      )),
      row!(settings::item(
        fl!("connection-status"),
        text(if conn_status {
          fl!("connected")
        } else {
          fl!("disconnected")
        })
      )),
    ))];

    let status_row = Row::with_children(status_elements)
      .align_y(Alignment::Center)
      .spacing(0);

    let enable_elements: Vec<Element<'_, Message>> = vec![Element::from(
      column!(
        row!(settings::item(
          fl!("enable-ssh"),
          toggler(self.ssh).on_toggle(Message::EnableSSH)
        )),
        row!(settings::item(
          fl!("accept-routes"),
          toggler(self.routes).on_toggle(Message::AcceptRoutes)
        )),
      )
      .spacing(5),
    )];

    let enable_row = Row::with_children(enable_elements);

    let taildrop_elements: Vec<Element<'_, Message>> = vec![Element::from(
      column!(
        row!(text(fl!("tail-drop"))).align_y(Alignment::Center),
        row!(
          column!(
            dropdown(
              &self.device_options,
              self.selected_device_idx,
              Message::DeviceSelected
            )
            .width(110),
          )
          .align_x(Horizontal::Left)
          .padding(5),
          horizontal_space().width(Length::Fill),
          column!(
            button::standard(fl!("select-files"))
              .on_press(Message::ChooseFiles)
              .width(220)
              .tooltip(fl!("select-files-tooltip"))
          )
          .align_x(Horizontal::Right)
          .padding(5)
        )
        .align_y(Alignment::Center)
        .spacing(25),
        row!(
          column!(if !self.send_files.is_empty() {
            button::standard(fl!("send-files"))
              .on_press(Message::SendFiles)
              .width(110)
              .tooltip(fl!("send-files-tooltip"))
          } else {
            button::standard(fl!("send-files"))
              .width(110)
              .tooltip(fl!("send-files-tooltip"))
          })
          .align_x(Horizontal::Left)
          .padding(5),
          horizontal_space().width(Length::Fill),
          column!(
            button::standard(fl!("receive-files"))
              .on_press(Message::ReceiveFiles)
              .width(220)
              .tooltip(fl!("receive-files-tooltip"))
          )
          .align_x(Horizontal::Right)
          .padding(5)
        )
        .align_y(Alignment::Center)
        .spacing(25)
      )
      .align_x(Alignment::Center),
    )];

    let taildrop_row = Row::with_children(taildrop_elements);

    let taildrop_status_elements: Vec<Element<'_, Message>> = vec![Element::from(column!(
      row!(text(fl!("send-receive-status"))
        .width(Length::Fill)
        .align_x(Horizontal::Center))
      .height(30)
      .align_y(Alignment::Center),
      row!(if !self.send_file_status.is_empty() {
        text(self.send_file_status.clone())
      } else if self.files_sent && self.selected_device != *"Select" {
        text(fl!("files-sent-success"))
      } else if self.selected_device == *"Select" && !self.files_sent {
        text(fl!("choose-device-first"))
      } else {
        text("")
      }),
      row!(text(self.receive_file_status.clone()))
    ))];

    let tx_rx_status_row = Row::with_children(taildrop_status_elements);

    let mut exit_node_elements: Vec<Element<'_, Message>> = Vec::new();

    let host_exit_node_col = column!(
      Element::from(
        if self.sel_exit_node_idx == Some(0) || self.sel_exit_node_idx.is_none() {
          if self.is_exit_node {
            toggler(self.is_exit_node)
              .label(fl!("disable-host-exit-node"))
              .on_toggle(Message::UpdateIsExitNode)
          } else {
            toggler(self.is_exit_node)
              .label(fl!("enable-host-exit-node"))
              .on_toggle(Message::UpdateIsExitNode)
          }
        } else {
          toggler(self.is_exit_node).label(fl!("enable-host-exit-node"))
        },
      ),
      Element::from(if self.is_exit_node {
        toggler(self.allow_lan)
          .label(fl!("allow-lan-access"))
          .on_toggle(Message::AllowExitNodeLanAccess)
      } else {
        toggler(self.allow_lan).label(fl!("allow-lan-access"))
      })
    )
    .spacing(5)
    .align_x(Alignment::Start);

    exit_node_elements.push(Element::from(
      column!(
        row!(text(fl!("exit-node"))
          .width(Length::Fill)
          .align_x(Horizontal::Center)),
        row!(
          column!(column!(
            text(fl!("selected-node"))
              .align_x(Alignment::Start)
              .align_y(Alignment::Center),
            dropdown(
              &self.avail_exit_nodes,
              self.sel_exit_node_idx,
              Message::ExitNodeSelected
            )
            .width(125)
          )
          .align_x(Alignment::Center))
          .padding(15)
          .align_x(Alignment::Center),
          column!(host_exit_node_col).padding(15)
        )
      )
      .spacing(10)
      .align_x(Alignment::Center),
    ));

    let exit_node_row = Row::with_children(exit_node_elements);

    let content_list = list_column()
      .padding(5)
      .spacing(0)
      .add(Element::from(status_row))
      .add(Element::from(enable_row))
      .add(settings::item(
        fl!("connected-label"),
        toggler(self.connect).on_toggle(Message::ConnectDisconnect),
      ))
      .add(Element::from(taildrop_row))
      .add(Element::from(tx_rx_status_row))
      .add(Element::from(exit_node_row));

    self.core.applet.popup_container(content_list).into()
  }
}
