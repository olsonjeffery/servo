var elem = document.documentElement.firstChild;

var start = (new Date()).getTime();
for (var i = 0; i < 1000000; i++) {
  var a = elem.nodeType;
}
window.alert((new Date()).getTime() - start);

/*start = new Date().getTime();
for (i = 0; i < 10000; i++)
  elem.width = i;
window.alert(new Date().getTime() - start);*/