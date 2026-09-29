class profile::web (
  String $docroot = '/var/www',
) {
  include profile::base
  if $facts['os']['family'] == 'RedHat' {
    $pkg = 'httpd'
  } else {
    $pkg = 'apache2'
  }
  package { $pkg:
    ensure => installed,
  }
  $cmd = lookup('web.healthcheck_cmd')
  exec { 'healthcheck':
    command => $cmd,
    path    => ['/bin', '/usr/bin'],
  }
}
