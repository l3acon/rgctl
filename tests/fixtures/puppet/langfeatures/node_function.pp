# Node, function, type alias, case/if control flow
type Profile::Port = Integer[1, 65535]

function profile::helpers::normalize($value) {
  $value
}

node 'web01' {
  include role::web
  if $facts['os']['family'] == 'RedHat' {
    include profile::yum
  } else {
    include profile::apt
  }
  case $facts['os']['family'] {
    'RedHat': { notify { 'rh': } }
    default:  { notify { 'other': } }
  }
}
