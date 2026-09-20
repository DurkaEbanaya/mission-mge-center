gtk_nocsd = shared_library(
  'gtk-nocsd',
  '../vendor/gtk-nocsd/Source/GTK-NoCSD.c',
  dependencies: dependency('libadwaita-1'),
  c_args: [
    '-Wall',
    '-Wextra',
    '-Wconversion',
    '-Wstrict-prototypes',
  ],
  link_args: ['-Wl,-e,GTKNoCSDMain'],
  soversion: '0',
  install: true,
  install_dir: get_option('libdir') / 'missioncenter',
)

wrapper_conf = configuration_data()
wrapper_conf.set('BINDIR', get_option('prefix') / get_option('bindir'))
wrapper_conf.set('GTK_NOCSD',
  get_option('prefix') / get_option('libdir') / 'missioncenter' / 'libgtk-nocsd.so.0')

configure_file(
  input: 'missioncenter-kde.in',
  output: 'missioncenter-kde',
  configuration: wrapper_conf,
  install: true,
  install_dir: get_option('bindir'),
  install_mode: 'rwxr-xr-x',
)

install_data(
  '../vendor/gtk-nocsd/LICENSE',
  install_dir: get_option('datadir') / 'licenses' / 'gtk-nocsd',
)
