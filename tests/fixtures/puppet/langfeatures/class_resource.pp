# Class, typed params, include, inherit, resources, ordering
class profile::nginx inherits profile::base (
  String $package_name = 'nginx',
) {
  include stdlib
  package { $package_name:
    ensure => installed,
  }
  service { 'nginx':
    ensure  => running,
    require => Package[$package_name],
  }
  Package[$package_name] -> Service['nginx']
}
