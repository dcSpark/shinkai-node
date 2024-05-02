// it should be inside external_subscriber_manager
// it should have a queue to upload files
// it should get notified for every new subscription that needs to handle (share or unshare) maybe that's it from ext_manager

// we should have a struct that encapsulates every file so we know if it's: sync, uploading, waiting, etc
// it should be similar to mirror's logic
// we need to generate a hash of the files and then a tree of the files. can we just use the hash of the vector resources? how can we check it in the other side?
// we upload vrkais so we can manage the files granularly
// we copy the folder structure of the PATH in the storage serve

// In the other end
// the user needs to specify that they want the http files
// the user asks the node for the subscription and current state of the files (it will indicate which ones are ready to be downloaded and which ones are not)
// the user will also need an http_download_manager.rs for this purpose
// should the user actually be in charge of checking diff? or should the node do it?
// it's pull so the user should be in charge of checking the diff
// files are downloading concurrently but also added concurrently to the VR (import of vrkai)

// we need to save the links somewhere. db then?
// delete all the links on unshare