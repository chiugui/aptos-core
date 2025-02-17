provider "kubernetes" {
  host                   = "https://${google_container_cluster.aptos.endpoint}"
  cluster_ca_certificate = base64decode(google_container_cluster.aptos.master_auth[0].cluster_ca_certificate)
  token                  = data.google_client_config.provider.access_token
}

resource "kubernetes_storage_class" "ssd" {
  metadata {
    name = "ssd"
  }
  storage_provisioner = "kubernetes.io/gce-pd"
  volume_binding_mode = "WaitForFirstConsumer"
  parameters = {
    type = "pd-ssd"
  }
}

provider "helm" {
  kubernetes {
    host                   = "https://${google_container_cluster.aptos.endpoint}"
    cluster_ca_certificate = base64decode(google_container_cluster.aptos.master_auth[0].cluster_ca_certificate)
    token                  = data.google_client_config.provider.access_token
  }
}

resource "helm_release" "fullnode" {
  count       = var.num_fullnodes
  name        = "${terraform.workspace}${count.index}"
  chart       = var.helm_chart
  max_history = 100
  wait        = false

  values = [
    jsonencode({
      chain = {
        era  = var.era
      }
      image = {
        tag = var.image_tag
      }
      nodeSelector = {
        "cloud.google.com/gke-nodepool" = "fullnodes"
      }
      storage = {
        class = kubernetes_storage_class.ssd.metadata[0].name
      }
      service = {
        type = "LoadBalancer"
      }
    }),
    jsonencode(var.fullnode_helm_values),
    jsonencode(var.fullnode_helm_values_list == {} ? {} : var.fullnode_helm_values_list[count.index]),
  ]

  set {
    name  = "timestamp"
    value = var.helm_force_update ? timestamp() : ""
  }
}

