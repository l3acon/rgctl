# Helper function used as a CALLS target for dashboard / callgraph gates.
function profile::helpers::ok() {
  true
}

class profile::base {
  $managed = true
  profile::helpers::ok()
}
